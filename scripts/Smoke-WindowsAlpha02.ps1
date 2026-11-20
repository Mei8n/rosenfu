# Isolated hosted runner only. Not a clean consumer Windows acceptance test.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string]$ArtifactDirectory,
    [ValidateRange(20, 180)] [int]$TimeoutSeconds = 90
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $env:RUNNER_OS -cne 'Windows' -or $env:RUNNER_ARCH -cne 'X64' -or
    $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
    $env:GITHUB_RUN_ID -notmatch '^[1-9][0-9]*$' -or $env:GITHUB_EVENT_NAME -notin @('push','pull_request')) {
    throw 'Only an isolated GitHub-hosted Windows x64 runner can execute this smoke'
}

function Require-PlainDirectory([string]$Path) {
    if ($Path -notmatch '^[a-zA-Z]:\\' -or -not [IO.Path]::IsPathFullyQualified($Path)) {
        throw "Expected an absolute local Windows directory: $Path"
    }
    $item = Get-Item -LiteralPath ([IO.Path]::GetFullPath($Path)) -Force
    if (-not $item.PSIsContainer) { throw "Expected a directory: $Path" }
    $full = $item.FullName
    while ($item) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Unexpected reparse point: $($item.FullName)" }
        $item = $item.Parent
    }
    return $full
}
function Require-PlainFile([string]$Path) {
    $file = Get-Item -LiteralPath $Path -Force
    if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Expected an ordinary file: $Path"
    }
    return $file
}

$temp = Require-PlainDirectory $env:RUNNER_TEMP
$workspace = Require-PlainDirectory $env:GITHUB_WORKSPACE
if ($temp -ceq [IO.Path]::GetPathRoot($temp)) { throw 'Runner temp must not be a volume root' }
if (-not [IO.Path]::IsPathFullyQualified($ArtifactDirectory)) {
    $ArtifactDirectory = Join-Path $workspace $ArtifactDirectory
}
$artifact = Require-PlainDirectory $ArtifactDirectory
if (-not $artifact.StartsWith($workspace.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Staged artifact directory must be below GITHUB_WORKSPACE'
}
$setup = @(Get-ChildItem -LiteralPath $artifact -File -Filter '*-windows-x64-setup.exe')
if ($setup.Count -ne 1) { throw 'Expected one staged installer' }
$installerFile = Require-PlainFile $setup[0].FullName
$sumFile = Require-PlainFile (Join-Path $artifact 'SHA256SUMS.txt')
$sum = (Get-Content -LiteralPath $sumFile.FullName -Raw).Trim()
$hash = (Get-FileHash -LiteralPath $installerFile.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
if ($sum -cne "$hash  $($installerFile.Name)") { throw 'Installer checksum mismatch' }
$signature = (Get-AuthenticodeSignature -LiteralPath $installerFile.FullName).Status
if ($signature -notin @('Valid','NotSigned')) { throw "Invalid installer Authenticode status: $signature" }
Write-Host "Installer Authenticode: $signature (NotSigned is allowed for this internal QA package)"

$qa = 'io.github.mei8n.rosenfu.alpha02qa'
$normal = 'io.github.mei8n.rosenfu'
$profileRoots = @((Require-PlainDirectory $env:APPDATA), (Require-PlainDirectory $env:LOCALAPPDATA)) | Select-Object -Unique
foreach ($p in $profileRoots) {
    foreach ($id in @($qa, $normal)) {
        if (Test-Path -LiteralPath (Join-Path $p $id)) { throw 'Expected fresh disposable Rosenfu user profile' }
    }
}
function Assert-NormalProfileUntouched {
    foreach ($p in $profileRoots) {
        if (Test-Path -LiteralPath (Join-Path $p $normal)) { throw 'QA touched the normal Rosenfu profile' }
    }
}

# NSIS /D must be last and unquoted, even with spaces.
$install = [IO.Path]::GetFullPath((Join-Path $temp ("rosenfu-qa-smoke-" + [guid]::NewGuid().ToString('N'))))
$installParent = [IO.Path]::GetDirectoryName($install)
if (-not $installParent.Equals($temp.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase) -or
    (Split-Path -Leaf $install) -notmatch '^rosenfu-qa-smoke-[a-f0-9]{32}$' -or (Test-Path -LiteralPath $install)) {
    throw 'Expected a new direct child of the verified runner temp'
}

function Stop-AndWait([Diagnostics.Process]$Process) {
    if ($Process.HasExited) { return }
    try { Stop-Process -InputObject $Process -Force -ErrorAction Stop }
    catch { if (-not $Process.HasExited) { throw } }
    if (-not $Process.WaitForExit(10000)) { throw "Process $($Process.Id) did not exit after forced stop" }
}
function Wait-Exit([Diagnostics.Process]$Process, [string]$Phase) {
    if (-not $Process.WaitForExit(1000 * $TimeoutSeconds)) {
        Stop-AndWait $Process
        throw "$Phase timed out"
    }
    if ($Process.ExitCode -ne 0) { throw "$Phase exit code: $($Process.ExitCode)" }
}
function Close-App([Diagnostics.Process]$Process) {
    if (-not $Process -or $Process.HasExited) { return }
    $null = $Process.CloseMainWindow()
    if (-not $Process.WaitForExit(10000)) { Stop-AndWait $Process }
}

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
function Find-ByName($Root, [string]$Name, $Type = $null) {
    if (-not $Root) { return $null }
    $nameCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty, $Name)
    $condition = $nameCondition
    if ($Type) {
        $typeCondition = [Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::ControlTypeProperty, $Type)
        $condition = [Windows.Automation.AndCondition]::new($nameCondition, $typeCondition)
    }
    return $Root.FindFirst([Windows.Automation.TreeScope]::Descendants, $condition)
}
function Find-InApp([Diagnostics.Process]$Process, [string]$Name, [string]$Dialog = '', $Type = $null) {
    $Process.Refresh()
    if ($Process.HasExited) { throw "App exited during smoke: $($Process.ExitCode)" }
    if ($Process.MainWindowHandle -eq [IntPtr]::Zero) { return $null }
    $root = [Windows.Automation.AutomationElement]::FromHandle($Process.MainWindowHandle)
    if ($Dialog) { $root = Find-ByName $root $Dialog }
    return Find-ByName $root $Name $Type
}
function Wait-For([string]$Description, [scriptblock]$Attempt) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt $TimeoutSeconds) {
        try {
            $value = & $Attempt
            if ($value) { return $value }
        } catch [Windows.Automation.ElementNotAvailableException] { }
        Start-Sleep -Milliseconds 500
    }
    throw "Timed out: $Description"
}
function Invoke-Button($Element) {
    $Element.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
}

$app = $null
$uninstaller = $null
$uninstallerHash = $null
$appPath = $null
$savePath = $null
$saveHash = $null
$manifestPath = $null
$manifestHash = $null
try {
    $installer = Start-Process -FilePath $installerFile.FullName -ArgumentList "/S /D=$install" -PassThru
    Wait-Exit $installer 'Installation'
    $null = Require-PlainDirectory $install
    $uninstaller = (Require-PlainFile (Join-Path $install 'uninstall.exe')).FullName
    $uninstallerHash = (Get-FileHash -LiteralPath $uninstaller -Algorithm SHA256).Hash
    $uninstallSignature = (Get-AuthenticodeSignature -LiteralPath $uninstaller).Status
    if ($uninstallSignature -notin @('Valid','NotSigned') -or
        ($signature -eq 'Valid' -and $uninstallSignature -ne 'Valid')) {
        throw "Unexpected uninstaller Authenticode status: $uninstallSignature"
    }
    $apps = @(Get-ChildItem -LiteralPath $install -File -Filter '*.exe' | Where-Object Name -ne 'uninstall.exe')
    if ($apps.Count -ne 1) { throw 'Expected one installed application executable' }
    $appPath = (Require-PlainFile $apps[0].FullName).FullName
    $registry = @(Get-ChildItem -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
        Where-Object {
            $entry = Get-ItemProperty -LiteralPath $_.PSPath
            $entry.DisplayName -ceq 'Rosenfu Alpha 0.2 Test' -and $entry.UninstallString -ceq ('"' + $uninstaller + '"')
        })
    if ($registry.Count -ne 1) { throw 'Expected one matching current-user QA uninstall registration' }
    $registryPath = $registry[0].PSPath

    $app = Start-Process -FilePath $appPath -PassThru
    $name = 'CI Release Smoke ' + [guid]::NewGuid().ToString('N').Substring(0, 12)
    $companyDialog = '会社名を設定して開始'
    $edit = [Windows.Automation.ControlType]::Edit
    $button = [Windows.Automation.ControlType]::Button
    $ready = Wait-For 'mounted company dialog and committed company entry' {
        $field = Find-InApp $app '開始する会社名' $companyDialog $edit
        if (-not $field -or -not $field.Current.IsEnabled) { return $null }
        $value = $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
        if ($value.Current.Value -cne $name) { $value.SetValue($name); return $null }
        $start = Find-InApp $app 'この会社で開始' $companyDialog $button
        if ($start -and $start.Current.IsEnabled) { return $start }
    }
    Invoke-Button $ready

    $saveRoot = Join-Path $env:APPDATA "$qa/prototype/saves"
    $manifest = Wait-For 'actual named QA save with matching world company' {
        if ($app.HasExited) { throw 'App exited while saving fresh smoke world' }
        $manifests = @(Get-ChildItem -LiteralPath $saveRoot -File -Recurse -Filter 'manifest.json' -ErrorAction SilentlyContinue)
        foreach ($file in $manifests) {
            $worldFile = Join-Path $file.DirectoryName 'world.rosenfu.json'
            if (-not (Test-Path -LiteralPath $worldFile -PathType Leaf)) { continue }
            $manifestContent = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json
            $worldContent = Get-Content -LiteralPath $worldFile -Raw | ConvertFrom-Json
            if ($manifestContent.name -ceq $name -and $worldContent.world.companies[0].name -ceq $name) { return $file }
        }
    }
    $savePath = (Require-PlainFile (Join-Path $manifest.DirectoryName 'world.rosenfu.json')).FullName
    $manifestPath = (Require-PlainFile $manifest.FullName).FullName
    Assert-NormalProfileUntouched
    Close-App $app

    $app = Start-Process -FilePath $appPath -PassThru
    $choose = Wait-For 'fresh-start load button after relaunch' {
        $control = Find-InApp $app '保存済みワールドを読み込む' $companyDialog $button
        if ($control -and $control.Current.IsEnabled) { return $control }
    }
    Invoke-Button $choose
    $load = Wait-For 'the exact saved world in the load dialog' {
        $control = Find-InApp $app $name 'ワールドを選択' $button
        if ($control -and $control.Current.IsEnabled) { return $control }
    }
    Invoke-Button $load
    $null = Wait-For 'same-world successful reload notice and closed picker' {
        $loaded = Find-InApp $app '保存したワールドを読み込みました。'
        $stillOpen = Find-InApp $app $name 'ワールドを選択' $button
        if ($loaded -and -not $stillOpen) { return $true }
    }
    Close-App $app
    Assert-NormalProfileUntouched
    $saveHash = (Get-FileHash -LiteralPath $savePath -Algorithm SHA256).Hash
    $manifestHash = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
} finally {
    Close-App $app
    if ($uninstaller -and $uninstallerHash -and (Test-Path -LiteralPath $uninstaller)) {
        $null = Require-PlainDirectory $install
        $null = Require-PlainFile $uninstaller
        if ((Get-FileHash -LiteralPath $uninstaller -Algorithm SHA256).Hash -cne $uninstallerHash) {
            throw 'Uninstaller changed since setup'
        }
        # _?= runs the actual NSIS uninstaller synchronously instead of spawning a copy,
        # and must be last/unquoted. Do not pass /DELETEAPPDATA.
        $remove = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$install" -PassThru
        Wait-Exit $remove 'Uninstaller'
        if (($appPath -and (Test-Path -LiteralPath $appPath)) -or
            ($registryPath -and (Test-Path -LiteralPath $registryPath))) {
            throw 'Application executable or registration still present after uninstall'
        }
        if ($saveHash -and ((Get-FileHash -LiteralPath $savePath -Algorithm SHA256).Hash -cne $saveHash -or
            (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash -cne $manifestHash)) {
            throw 'Ordinary uninstall modified the QA save or manifest'
        }
        Assert-NormalProfileUntouched
    }
}
Write-Host "Installed frontend, save/reload, ordinary uninstall, and QA profile isolation verified: $hash"
