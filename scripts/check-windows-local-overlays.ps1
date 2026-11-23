# Hosted validation only: build, read PE metadata and manifest, print receipts.
# Do not install, archive or publish any executable.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_OS -cne 'Windows' -or
    $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
    $env:GITHUB_REPOSITORY -cne 'Mei8n/rosenfu' -or
    $env:GITHUB_EVENT_NAME -cne 'pull_request' -or
    $env:GITHUB_HEAD_REF -cne 'work/alpha02-playtest' -or
    $env:GITHUB_BASE_REF -cne 'dev/alpha-0.2-integration') {
    throw 'This check is restricted to the same-repository hosted Windows PR job'
}
$workspace = Get-Item -LiteralPath $env:GITHUB_WORKSPACE -Force
if (-not $workspace.PSIsContainer -or ($workspace.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
    (Get-Location).ProviderPath -ne $workspace.FullName) {
    throw 'Expected the actual GitHub checkout root as the working directory'
}
$checkout = (git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $checkout -cnotmatch '^[0-9a-f]{40}$' -or
    $checkout -cne $env:GITHUB_SHA) { throw 'Exact workflow checkout mismatch' }

$temp = Get-Item -LiteralPath $env:RUNNER_TEMP -Force
if (-not $temp.PSIsContainer -or ($temp.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Invalid runner temporary root'
}
$build = Join-Path $temp.FullName 'rosenfu-local-overlay-compile'
# Fails if a previous run/step left the same directory. No use of a payload cache.
$null = New-Item -ItemType Directory -Path $build -ErrorAction Stop

$project = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
$base = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
$qa = Get-Content -LiteralPath 'src-tauri/tauri.alpha02-qa.conf.json' -Raw | ConvertFrom-Json
$version = [string]$qa.version
$product = 'Rosenfu Alpha 0.2 Test'
if ($version -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$' -or
    $version -cne $project.version -or $version -cne $base.version -or
    $qa.productName -cne $product -or $qa.identifier -cne 'io.github.mei8n.rosenfu.alpha02qa') {
    throw 'Unexpected QA build identity'
}

$sdkBase = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
$sdkMt = @(Get-ChildItem -LiteralPath $sdkBase -Directory |
    Where-Object { $_.Name -match '^10\.\d+\.\d+\.\d+$' } |
    Sort-Object { [version]$_.Name } -Descending |
    ForEach-Object { Join-Path $_.FullName 'x64\mt.exe' } |
    Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
if ($sdkMt.Count -lt 1) { throw 'Hosted Windows SDK manifest-reader missing' }
$mt = Get-Item -LiteralPath $sdkMt[0] -Force
if ($mt.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected redirect in SDK manifest reader' }

function Verify-Overlay($Name, $InstallMode, $Privilege) {
    $configPath = "src-tauri/tauri.alpha02-local-$Name.conf.json"
    $template = Get-Item -LiteralPath "src-tauri/nsis/alpha02-local-$Name.nsi" -Force
    if ($template.PSIsContainer -or ($template.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Expected an ordinary tracked NSIS template'
    }
    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    if ($config.bundle.windows.nsis.installMode -cne $InstallMode -or
        $config.bundle.windows.nsis.template -cne "nsis/alpha02-local-$Name.nsi") {
        throw 'Wrong local build overlay'
    }
    $env:CARGO_TARGET_DIR = Join-Path $build $Name
    $null = New-Item -ItemType Directory -Path $env:CARGO_TARGET_DIR -ErrorAction Stop
    & npm run tauri -- build --ci --target x86_64-pc-windows-msvc --bundles nsis --config src-tauri/tauri.alpha02-qa.conf.json --config $configPath -- --locked | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Build failed: $Name" }
    $out = Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc\release\bundle\nsis'
    $setup = @(Get-ChildItem -LiteralPath $out -Filter '*-setup.exe' -File)
    if ($setup.Count -ne 1 -or $setup[0].Length -le 0 -or
        ($setup[0].Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        $setup[0].VersionInfo.ProductName -cne $product -or
        $setup[0].VersionInfo.ProductVersion -cne $version) {
        throw "Expected one exact product/version setup executable: $Name"
    }
    $manifestPath = Join-Path $build "$Name.manifest"
    & $mt.FullName -nologo "-inputresource:$($setup[0].FullName);#1" "-out:$manifestPath" | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Could not read built PE manifest: $Name" }

    # Explicitly do not resolve externals. Accept exactly one UAC declaration.
    $xmlSettings = [Xml.XmlReaderSettings]::new()
    $xmlSettings.DtdProcessing = [Xml.DtdProcessing]::Prohibit
    $xmlSettings.XmlResolver = $null
    $reader = [Xml.XmlReader]::Create($manifestPath, $xmlSettings)
    try {
        $doc = [Xml.XmlDocument]::new()
        $doc.XmlResolver = $null
        $doc.Load($reader)
    } finally { $reader.Dispose() }
    $ns = [Xml.XmlNamespaceManager]::new($doc.NameTable)
    $ns.AddNamespace('asm3', 'urn:schemas-microsoft-com:asm.v3')
    $levels = $doc.SelectNodes('//asm3:requestedExecutionLevel', $ns)
    if ($levels.Count -ne 1 -or $levels[0].GetAttribute('level') -cne $Privilege -or
        $levels[0].GetAttribute('uiAccess') -cne 'false') {
        throw "Built setup privilege does not match requested scope: $Name"
    }
    $hash = (Get-FileHash -LiteralPath $setup[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "local-overlay=$Name source=$checkout version=$version privilege=$Privilege sha256=$hash"
    return $hash
}
try {
    $admin = Verify-Overlay admin perMachine requireAdministrator
    $user = Verify-Overlay user currentUser asInvoker
    if ($admin -ceq $user) { throw 'Expected distinct current-user and machine binaries' }
} finally {
    Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
}
