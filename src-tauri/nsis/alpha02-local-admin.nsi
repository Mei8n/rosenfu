; Local-only Rosenfu QA variant of Tauri's MIT-licensed NSIS template.
; Base: tauri-cli-v2.12.4, 993038c49185307dfc895ab7657d77ca0d5ed7c7.
; Copyright (c) 2017 - Present Tauri Apps Contributors. See LICENSE-Tauri-MIT.
Unicode true
ManifestDPIAware true
; Add in `dpiAwareness` `PerMonitorV2` to manifest for Windows 10 1607+ (note this should not affect lower versions since they should be able to ignore this and pick up `dpiAware` `true` set by `ManifestDPIAware true`)
; Currently undocumented on NSIS's website but is in the Docs folder of source tree, see
; https://github.com/kichik/nsis/blob/5fc0b87b819a9eec006df4967d08e522ddd651c9/Docs/src/attributes.but#L286-L300
; https://github.com/tauri-apps/tauri/pull/10106
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  ; Set the compression algorithm. We default to LZMA.
  SetCompressor /SOLID "{{compression}}"
!endif

; Keep above !include to stay ahead of any plugin command
; see https://github.com/tauri-apps/tauri/pull/15422#discussion_r3289239624
{{#if signed_plugins_path}}
!addplugindir "{{signed_plugins_path}}"
{{/if}}

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include "utils.nsh"
!include "FileAssociation.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "Win\RestartManager.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

{{#if installer_hooks}}
!include "{{installer_hooks}}"
{{/if}}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!define LICENSE "{{license}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define UNINSTALLERICON "{{uninstaller_icon}}"
!define UNINSTALLERHEADERIMAGE "{{uninstaller_header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define STARTMENUFOLDER "{{start_menu_folder}}"

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
Var OldMainBinaryName

Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"

; We don't actually use this value as default install path,
; it's just for nsis to append the product name folder in the directory selector
; https://nsis.sourceforge.io/Reference/InstallDir
; Only the local x64 elevated profile may use this template.
!if "${INSTALLMODE}" != "perMachine"
  !error "Rosenfu local admin template requires perMachine"
!endif
!if "${ARCH}" != "x64"
  !error "Rosenfu local admin template requires x64"
!endif
!if "${BUNDLEID}" != "io.github.mei8n.rosenfu.alpha02qa"
  !error "Apply the existing Rosenfu Windows QA profile before the local admin overlay"
!endif
!if "${PRODUCTNAME}" != "Rosenfu Alpha 0.2 Test"
  !error "Rosenfu local admin template requires the stable Windows QA product name"
!endif
; No alternate uninstaller icon: the pinned NSIS stub's optional icon
; rewrite precedes its checked extraction. Keep the plain guarded writer.
!if "${UNINSTALLERICON}" != ""
  !error "Local admin layout does not support a custom uninstaller icon"
!endif
; The pinned Tauri NSIS bundler renders the configured skip mode as "".
; Never admit a profile that can bundle, fetch, or run a runtime installer.
!if "${INSTALLWEBVIEW2MODE}" != ""
  !error "Rosenfu local admin template requires webviewInstallMode: skip"
!endif
{{#if resources}}
!error "Review the elevated local path checks before adding resources"
{{/if}}
{{#if binaries}}
!error "Review the elevated local path checks before adding sidecars"
{{/if}}
{{#if installer_hooks}}
!error "Review the elevated local path checks before adding installer hooks"
{{/if}}
!define ROSENFU_LOCAL_LAYOUT "rosenfu-local-machine-v1"
!define ROSENFU_LOCAL_DIR "$PROGRAMFILES64\rosenfu"
!define PLACEHOLDER_INSTALL_DIR "placeholder\rosenfu"
Var RosenfuExisting
Var RosenfuRegisteredDir
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

# additional plugins
!addplugindir "${ADDITIONALPLUGINSPATH}"

; Uninstaller signing command
!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

; Handle install mode, `perUser`, `perMachine` or `both`
!if "${INSTALLMODE}" == "perMachine"
  RequestExecutionLevel admin
!endif

!if "${INSTALLMODE}" == "currentUser"
  RequestExecutionLevel user
!endif

!if "${INSTALLMODE}" == "both"
  !define MULTIUSER_MUI
  !define MULTIUSER_INSTALLMODE_INSTDIR "${PRODUCTNAME}"
  !define MULTIUSER_INSTALLMODE_COMMANDLINE
  !if "${ARCH}" == "x64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !else if "${ARCH}" == "arm64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !endif
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${UNINSTKEY}"
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "CurrentUser"
  !define MULTIUSER_INSTALLMODEPAGE_SHOWUSERNAME
  !define MULTIUSER_INSTALLMODE_FUNCTION RestorePreviousInstallLocation
  !define MULTIUSER_EXECUTIONLEVEL Highest
  !include MultiUser.nsh
!endif

; Installer icon
!if "${INSTALLERICON}" != ""
  !define MUI_ICON "${INSTALLERICON}"
!endif

; Installer sidebar image
!if "${SIDEBARIMAGE}" != ""
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif

; Enable header images for installer and uninstaller pages when either image is configured.
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!else if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!endif

; Installer header image
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!endif

; Uninstaller header image
!if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"
!endif

; Uninstaller icon
!if "${UNINSTALLERICON}" != ""
  !define MUI_UNICON "${UNINSTALLERICON}"
!endif

; Define registry key to store installer language
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

; Installer pages, must be ordered as they appear
; 1. Welcome Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_WELCOME

; 2. License Page (if defined)
!if "${LICENSE}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!endif

; 3. Install mode (if it is set to `both`)
!if "${INSTALLMODE}" == "both"
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MULTIUSER_PAGE_INSTALLMODE
!endif

; No legacy maintenance page: local admin setup never executes registry
; uninstall commands. Its own verified machine installs update in place.

; Interactive first installs can choose a new directory in a protected local path.
; Updates stay in the registered location.
!define MUI_PAGE_CUSTOMFUNCTION_PRE RosenfuDirectoryPre
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE RosenfuDirectoryLeave
!insertmacro MUI_PAGE_DIRECTORY

; 6. Start menu shortcut page
Var AppStartMenuFolder
!if "${STARTMENUFOLDER}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !define MUI_STARTMENUPAGE_DEFAULTFOLDER "${STARTMENUFOLDER}"
!else
  !define MUI_PAGE_CUSTOMFUNCTION_PRE Skip
!endif
!insertmacro MUI_PAGE_STARTMENU Application $AppStartMenuFolder

; 7. Installation page
!insertmacro MUI_PAGE_INSTFILES

; 8. Finish page
;
; Don't auto jump to finish page after installation page,
; because the installation page has useful info that can be used debug any issues with the installer.
!define MUI_FINISHPAGE_NOAUTOCLOSE
; Use show readme button in the finish page as a button create a desktop shortcut
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(createDesktop)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateOrUpdateDesktopShortcut
; Show run app after installation.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_FINISH

Function RunMainBinary
  nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
FunctionEnd

; Uninstaller Pages
; Standard uninstall confirmation, with no per-user data deletion.
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM

; 2. Uninstalling Page
!insertmacro MUI_UNPAGE_INSTFILES

;Languages
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
  !include "{{this}}"
{{/each}}

Function .onInit
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NS" $NoShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  !if "${DISPLAYLANGUAGESELECTOR}" == "true"
    !insertmacro MUI_LANGDLL_DISPLAY
  !endif

  !insertmacro SetContext

  ${If} $INSTDIR != "${PLACEHOLDER_INSTALL_DIR}"
  ${AndIf} $INSTDIR != "${ROSENFU_LOCAL_DIR}"
    StrCpy $R9 "管理者用 QA 版は Program Files の rosenfu フォルダーにのみインストールできます。"
    Call RosenfuRefuse
  ${EndIf}
  StrCpy $INSTDIR "${ROSENFU_LOCAL_DIR}"
  Call RosenfuReadRegistration
  ${If} $RosenfuExisting == 1
    StrCpy $INSTDIR "$RosenfuRegisteredDir"
  ${EndIf}
FunctionEnd

Function RosenfuDirectoryPre
  ${If} $RosenfuExisting == 1
  ${OrIf} $PassiveMode == 1
    Abort
  ${EndIf}
FunctionEnd

Function RosenfuDirectoryLeave
  Call RosenfuValidateTarget
  ${If} $R8 != 1
    MessageBox MB_ICONSTOP|MB_OK "$R9"
    Abort
  ${EndIf}
FunctionEnd

; Read only the exact QA keys in the effective user's HKCU. A credentials
; prompt may select a different account; this does not inspect other hives.
!macro RosenfuRequireNoCurrentUserQa key flags
  System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "${key}", i 0, i ${flags}, *p .r0) i .r1'
  ${If} $1 = 0
    System::Call 'advapi32::RegCloseKey(p r0)'
  ${EndIf}
  ${If} $1 != 2
    StrCpy $R9 "現在のユーザーに別の QA 版が登録されているか、確認できません。元の QA 版とセーブを確認してください。旧 QA の登録だけが残っている場合は手順書を参照してください。このセットアップは旧版に触れません。"
    Call RosenfuRefuse
  ${EndIf}
!macroend

Function RosenfuReadRegistration
  StrCpy $RosenfuExisting 0
  StrCpy $RosenfuRegisteredDir ""
  !insertmacro RosenfuRequireNoCurrentUserQa "${UNINSTKEY}" 0x20119
  !insertmacro RosenfuRequireNoCurrentUserQa "${UNINSTKEY}" 0x20219
  !insertmacro RosenfuRequireNoCurrentUserQa "Software\Microsoft\Windows\CurrentVersion\Uninstall\${BUNDLEID}" 0x20119
  !insertmacro RosenfuRequireNoCurrentUserQa "Software\Microsoft\Windows\CurrentVersion\Uninstall\${BUNDLEID}" 0x20219
  ReadRegStr $0 HKLM "${UNINSTKEY}" "RosenfuLocalLayout"
  ReadRegStr $1 HKLM "${UNINSTKEY}" "UninstallString"
  ReadRegStr $2 HKLM "${UNINSTKEY}" "DisplayName"
  ReadRegStr $3 HKLM "${UNINSTKEY}" "InstallLocation"
  ReadRegStr $4 HKLM "${MANUPRODUCTKEY}" ""
  ReadRegStr $5 HKLM "${UNINSTKEY}" "MainBinaryName"
  ReadRegStr $6 HKLM "${UNINSTKEY}" "DisplayVersion"
  ${If} "$0$1$2$3$4$5$6" == ""
    Return
  ${EndIf}
  ${If} $0 != "${ROSENFU_LOCAL_LAYOUT}"
  ${OrIf} $2 != "${PRODUCTNAME}"
  ${OrIf} $4 == ""
  ${OrIf} $3 != '$\"$4$\"'
  ${OrIf} $1 != '$\"$4\uninstall.exe$\"'
  ${OrIf} $5 != "${MAINBINARYNAME}.exe"
  ${OrIf} $6 == ""
    StrCpy $R9 "既存の PC 共通インストール情報を確認できません。書き換えずに中止します。"
    Call RosenfuRefuse
  ${EndIf}
  nsis_tauri_utils::SemverCompare "${VERSION}" "$6"
  Pop $7
  ${If} $7 != 0
  ${AndIf} $7 != 1
    StrCpy $R9 "インストール済みの QA 版より古いか、バージョンを確認できないため中止します。"
    Call RosenfuRefuse
  ${EndIf}
  StrCpy $RosenfuRegisteredDir "$4"
  StrCpy $RosenfuExisting 1
FunctionEnd

Function RosenfuRefuse
  DetailPrint "$R9"
  IfSilent +2
    MessageBox MB_ICONSTOP|MB_OK "$R9"
  SetErrorLevel 2
  Quit
FunctionEnd

; The machine installer uses protected Program Files roots and the stock
; payload writer. In the owned directory, refuse either installed file
; if the opened object is not an ordinary single-link file.
Function RosenfuCheckUniqueInstalledFile
  StrCpy $R8 0
  ; FILE_READ_ATTRIBUTES, share read/write/delete, OPEN_EXISTING,
  ; FILE_FLAG_OPEN_REPARSE_POINT. Inspect the object, not a link target.
  System::Call 'kernel32::CreateFileW(w r0, i 128, i 7, p 0, i 3, i 0x200000, p 0) p .r3'
  StrCmp $3 -1 unique_check_done
  System::Alloc 52
  Pop $4
  StrCmp $4 0 unique_check_close
  StrCmp $4 "" unique_check_close
  System::Call 'kernel32::GetFileInformationByHandle(p r3, p r4) i .r1'
  StrCmp $1 0 unique_check_free
  ; BY_HANDLE_FILE_INFORMATION has 13 DWORD words. nNumberOfLinks is
  ; word 11 (ten words precede it). Read attributes from the same handle.
  System::Call '*$4(i .r2, i, i, i, i, i, i, i, i, i, i .r5)'
  IntOp $2 $2 & 0x410
  StrCmp $2 0 0 unique_check_free
  StrCmp $5 1 0 unique_check_free
  StrCpy $R8 1
  unique_check_free:
    System::Free $4
  unique_check_close:
    System::Call 'kernel32::CloseHandle(p r3)'
  unique_check_done:
FunctionEnd

; Validate paths for elevated operations without modifying chosen parent ACLs.
Var RosenfuAclPath
Var RosenfuAclIndex
Var RosenfuAclRoot
Var RosenfuAclOk
Var RosenfuAclObject
Var RosenfuAclMode

!macro RosenfuAclDirectory prefix
Function ${prefix}RosenfuAclObjectIsProtected
  ; Input: absolute, fixed-drive path, previously checked ancestor.
  ; The root may grant create-only access; descendants and files may not.
  StrCpy $RosenfuAclOk 0
  StrCpy $6 0
  System::Call 'advapi32::GetNamedSecurityInfoW(w "$RosenfuAclObject", i 1, i 5, *p .r1, p 0, *p .r2, p 0, *p .r3) i .r0'
  StrCmp $0 0 0 acl_done
  StrCmp $1 0 acl_free
  StrCmp $2 0 acl_free
  ; Only owners who are outside ordinary user tokens may change the DACL.
  StrCpy $7 $1
  Call ${prefix}RosenfuTrustedSid
  StrCmp $8 1 0 acl_free
  System::Alloc 12
  Pop $6
  StrCmp $6 0 acl_free
  StrCmp $6 "" acl_free
  System::Call 'advapi32::GetAclInformation(p r2, p r6, i 12, i 2) i .r0'
  StrCmp $0 0 acl_free
  System::Call '*$6(i .r5)'
  StrCmp $5 0 acl_free
  StrCpy $4 0
  acl_next:
    IntCmp $4 $5 acl_pass 0 acl_free
    System::Call 'advapi32::GetAce(p r2, i r4, *p .r7) i .r0'
    StrCmp $0 0 acl_free
    System::Call '*$7(i .r0, i .r9)'
    IntOp $1 $0 & 0xff
    ; Never treat an unrecognized or conditional ACE as a proof.
    StrCmp $1 1 acl_advance ; ACCESS_DENIED_ACE can only reduce access.
    StrCmp $1 0 0 acl_free ; ACCESS_ALLOWED_ACE.
    IntOp $1 $0 & 0x800
    StrCmp $1 0 0 acl_advance ; INHERIT_ONLY applies to descendants.
    StrCmp $RosenfuAclMode "root" 0 +3
      IntOp $9 $9 & 0x500d0150
      Goto acl_masked
    IntOp $9 $9 & 0x500d0156
    acl_masked:
    StrCmp $9 0 acl_advance
    IntOp $7 $7 + 8 ; ACCESS_ALLOWED_ACE.SidStart.
    Call ${prefix}RosenfuTrustedSid
    StrCmp $8 1 0 acl_free
    acl_advance:
    IntOp $4 $4 + 1
    Goto acl_next
  acl_pass:
    StrCpy $RosenfuAclOk 1
  acl_free:
    StrCmp $6 0 +3
    StrCmp $6 "" +2
      System::Free $6
    StrCmp $3 0 +2
      System::Call 'kernel32::LocalFree(p r3)'
  acl_done:
FunctionEnd

; Use SID strings rather than localized names. Scratch: $0, $1, $7-$9.
Function ${prefix}RosenfuTrustedSid
  StrCpy $8 0
  System::Call 'advapi32::ConvertSidToStringSidW(p r7, *p .r9) i .r0'
  StrCmp $0 0 sid_done
  System::Call 'kernel32::lstrcpynW(w .r1, p r9, i 100) p'
  System::Call 'kernel32::LocalFree(p r9)'
  StrCmp $1 "S-1-5-18" sid_trusted
  StrCmp $1 "S-1-5-32-544" sid_trusted
  StrCmp $1 "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464" sid_trusted
  Goto sid_done
  sid_trusted:
    StrCpy $8 1
  sid_done:
FunctionEnd

; The caller supplies the longest existing directory. Check from the volume
; root downward, so a mutable child cannot stand in for a verified ancestor.
Function ${prefix}RosenfuCheckCustomTree
  StrCpy $RosenfuAclOk 0
  StrCpy $RosenfuAclRoot "$RosenfuAclPath" 3
  StrCpy $0 "$RosenfuAclRoot" 2 1
  StrCmp $0 ":\" 0 tree_bad
  System::Call 'kernel32::GetDriveTypeW(w "$RosenfuAclRoot") i .r0'
  StrCmp $0 3 0 tree_bad
  ClearErrors
  GetFullPathName $0 "$RosenfuAclPath"
  IfErrors tree_bad
  StrCmpS $0 "$RosenfuAclPath" 0 tree_bad
  StrCpy $RosenfuAclObject "$RosenfuAclRoot"
  StrCpy $RosenfuAclMode "root"
  Call ${prefix}RosenfuCheckFolder
  StrCmp $RosenfuAclOk 1 0 tree_bad
  StrCpy $RosenfuAclIndex 3
  tree_next:
    StrCpy $0 "$RosenfuAclPath" 1 $RosenfuAclIndex
    StrCmp $0 "" tree_last
    StrCmp $0 "\" tree_component
    IntOp $RosenfuAclIndex $RosenfuAclIndex + 1
    Goto tree_next
  tree_component:
    StrCpy $RosenfuAclObject "$RosenfuAclPath" $RosenfuAclIndex
    StrCpy $RosenfuAclMode "normal"
    Call ${prefix}RosenfuCheckFolder
    StrCmp $RosenfuAclOk 1 0 tree_bad
    IntOp $RosenfuAclIndex $RosenfuAclIndex + 1
    Goto tree_next
  tree_last:
    StrCmp "$RosenfuAclPath" "$RosenfuAclRoot" tree_done
    StrCpy $RosenfuAclObject "$RosenfuAclPath"
    StrCpy $RosenfuAclMode "normal"
    Call ${prefix}RosenfuCheckFolder
  tree_done:
    Return
  tree_bad:
    StrCpy $RosenfuAclOk 0
FunctionEnd

Function ${prefix}RosenfuCheckFolder
  StrCpy $RosenfuAclOk 0
  System::Call 'kernel32::GetFileAttributesW(w "$RosenfuAclObject") i .r0'
  IntOp $0 $0 & 0x410
  StrCmp $0 16 0 folder_done
  Call ${prefix}RosenfuAclObjectIsProtected
  folder_done:
FunctionEnd

Function ${prefix}RosenfuCheckInstalledAcl
  StrCpy $RosenfuAclMode "normal"
  StrCpy $RosenfuAclObject "$INSTDIR\${MAINBINARYNAME}.exe"
  Call ${prefix}RosenfuAclObjectIsProtected
  StrCmp $RosenfuAclOk 1 0 installed_acl_done
  StrCpy $RosenfuAclObject "$INSTDIR\uninstall.exe"
  Call ${prefix}RosenfuAclObjectIsProtected
  installed_acl_done:
FunctionEnd
!macroend
!insertmacro RosenfuAclDirectory ""
!insertmacro RosenfuAclDirectory "un."

; Validated fresh installs require a new directory; even an empty existing
; directory is not adopted. Recheck right before creating/copying files.
Function RosenfuValidateTarget
  StrCpy $R8 0
  StrCpy $R9 "固定ローカルドライブ上で、管理者が保護している親フォルダーの下に、新しい rosenfu フォルダーを指定してください。既存フォルダーやリンクは使用できません。"
  StrCpy $0 "$INSTDIR" 8 -8
  StrCmp $0 "\rosenfu" 0 rosenfu_invalid
  ${If} $RosenfuExisting == 1
  ${AndIf} $INSTDIR != $RosenfuRegisteredDir
    Return
  ${EndIf}
  StrCpy $0 "$INSTDIR" 2 1
  ${IfThen} $0 != ":\" ${|} Return ${|}
  StrCpy $6 "$INSTDIR" 3
  ${IfThen} $6 == $INSTDIR ${|} Return ${|}
  System::Call 'kernel32::GetDriveTypeW(w r6) i .r1'
  ${IfThen} $1 != 3 ${|} Return ${|} ; DRIVE_FIXED
  ${GetParent} "$INSTDIR" $0
  rosenfu_ancestor:
    StrCmp $0 "" rosenfu_invalid
    StrCpy $1 "$0\" ; GetParent returns C: for a direct root child.
    StrCmp $1 $6 rosenfu_leaf
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410 ; directory (0x10) and reparse point (0x400)
    StrCmp $2 16 0 rosenfu_invalid
    ${GetParent} "$0" $3
    StrCmp $3 $0 rosenfu_invalid
    StrCpy $0 "$3"
    Goto rosenfu_ancestor
  rosenfu_leaf:
  ; NSIS 3.13 GetFullPathName looks up its final component and reports
  ; an error for a missing name. Canonicalize the now-checked existing
  ; parent for a fresh install; the fixed product leaf must not exist.
  ${If} $RosenfuExisting == 0
    ${GetParent} "$INSTDIR" $0
  ${Else}
    StrCpy $0 "$INSTDIR"
  ${EndIf}
  ClearErrors
  GetFullPathName $7 "$0"
  ${If} ${Errors}
  ${OrIf} $7 != $0
    Return
  ${EndIf}
  ${If} $INSTDIR != "${ROSENFU_LOCAL_DIR}"
    StrCpy $RosenfuAclPath "$0"
    Call RosenfuCheckCustomTree
    ${IfThen} $RosenfuAclOk != 1 ${|} Return ${|}
    ${If} $RosenfuExisting == 1
      Call RosenfuCheckInstalledAcl
      ${IfThen} $RosenfuAclOk != 1 ${|} Return ${|}
    ${EndIf}
  ${EndIf}
  StrCpy $0 "$INSTDIR"
  System::Call 'kernel32::GetFileAttributesW(w r0) i .r1 ?e'
  Pop $2
  ${If} $RosenfuExisting == 0
    ; With an existing checked parent, only FILE_NOT_FOUND permits creation.
    ${If} $1 == -1
    ${AndIf} $2 == 2
      StrCpy $R8 1
    ${EndIf}
    Return
  ${EndIf}
  StrCpy $R9 "インストール先または実行ファイルが登録済みの QA 版と一致しないため中止します。"
  IntOp $2 $1 & 0x410
  ${IfThen} $2 != 16 ${|} Return ${|}
  StrCpy $0 "$INSTDIR\${MAINBINARYNAME}.exe"
  Call RosenfuCheckUniqueInstalledFile
  ${IfThen} $R8 != 1 ${|} Return ${|}
  StrCpy $0 "$INSTDIR\uninstall.exe"
  Call RosenfuCheckUniqueInstalledFile
  rosenfu_invalid:
FunctionEnd

; Microsoft documents this machine Evergreen registration for 64-bit Windows.
; SetContext already selected the 64-bit registry view for our x64 bundle.
; Do not treat the elevated caller's HKCU as the eventual app user's runtime.
Function RosenfuRequireMachineWebView2
  ClearErrors
  ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  IfErrors rosenfu_webview_missing
  StrCmp $4 "" rosenfu_webview_missing
  ${VersionCompare} "$4" "0.0.0.0" $R0
  StrCmp $R0 1 0 rosenfu_webview_missing
  !if "${MINIMUMWEBVIEW2VERSION}" != ""
    ${VersionCompare} "${MINIMUMWEBVIEW2VERSION}" "$4" $R0
    StrCmp $R0 1 rosenfu_webview_missing
  !endif
  Return

  rosenfu_webview_missing:
    StrCpy $R9 "Microsoft Edge WebView2 Evergreen Runtime のシステム全体へのインストールまたは更新が必要です。Microsoft 公式の x64 ランタイムを管理者としてインストールした後、このセットアップをやり直してください。"
    Call RosenfuRefuse
FunctionEnd

Section Install
  Call RosenfuReadRegistration
  Call RosenfuValidateTarget
  ${IfThen} $R8 != 1 ${|} Call RosenfuRefuse ${|}
  Call RosenfuRequireMachineWebView2
  ${If} $RosenfuExisting == 0
    System::Call 'kernel32::CreateDirectoryW(w "$INSTDIR", p 0) i .r1'
    StrCmp $1 0 rosenfu_payload_failed
  ${EndIf}
  ${If} $INSTDIR != "${ROSENFU_LOCAL_DIR}"
    StrCpy $RosenfuAclPath "$INSTDIR"
    Call RosenfuCheckCustomTree
    StrCmp $RosenfuAclOk 1 0 rosenfu_payload_failed
  ${EndIf}
  ClearErrors
  SetOutPath $INSTDIR
  IfErrors rosenfu_payload_failed

  !ifmacrodef NSIS_HOOK_PREINSTALL
    !insertmacro NSIS_HOOK_PREINSTALL
  !endif

  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  ; Mandatory payload for this install. Treat any recoverable extraction
  ; error as fatal, including the blocked/locked-file case.
  SetOverwrite on
  AllowSkipFiles off
  ClearErrors
  File "${MAINBINARYSRCPATH}"
  IfErrors rosenfu_payload_failed

  ; Copy resources
  {{#each resources_dirs}}
    ClearErrors
    CreateDirectory "$INSTDIR\\{{this}}"
    IfErrors rosenfu_payload_failed
  {{/each}}
  {{#each resources}}
    ClearErrors
    File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
    IfErrors rosenfu_payload_failed
  {{/each}}

  ; Copy external binaries
  {{#each binaries}}
    ClearErrors
    File /a "/oname={{this}}" "{{no-escape @key}}"
    IfErrors rosenfu_payload_failed
  {{/each}}

  ; Write the uninstaller before any application registration.
  ClearErrors
  WriteUninstaller "$INSTDIR\uninstall.exe"
  IfErrors rosenfu_payload_failed
  StrCpy $0 "$INSTDIR\${MAINBINARYNAME}.exe"
  System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
  IntOp $2 $1 & 0x410
  StrCmp $2 0 0 rosenfu_payload_failed
  StrCpy $0 "$INSTDIR\uninstall.exe"
  System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
  IntOp $2 $1 & 0x410
  StrCmp $2 0 0 rosenfu_payload_failed

  ; Create file associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
       !insertmacro APP_ASSOCIATE "{{ext}}" "{{or association.name ext}}" "{{association-description association.description ext}}" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0" "Open with ${PRODUCTNAME}" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
    {{/each}}
  {{/each}}

  ; Register deep links
  {{#each deep_link_protocols as |protocol| ~}}
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "URL Protocol" ""
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "" "URL:${BUNDLEID} protocol"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
  {{/each}}

  ${If} $INSTDIR != "${ROSENFU_LOCAL_DIR}"
    Call RosenfuCheckInstalledAcl
    StrCmp $RosenfuAclOk 1 0 rosenfu_payload_failed
  ${EndIf}

  ; Save $INSTDIR in registry for future installations
  WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR

  !if "${INSTALLMODE}" == "both"
    ; Save install mode to be selected by default for the next installation such as updating
    ; or when uninstalling
    WriteRegStr SHCTX "${UNINSTKEY}" $MultiUser.InstallMode 1
  !endif

  ; The local layout never adopts a different registered executable.
  StrCpy $OldMainBinaryName "${MAINBINARYNAME}.exe"

  ; Save current MAINBINARYNAME for future updates
  WriteRegStr SHCTX "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"

  ; Registry information for add/remove programs
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"

  ${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
  IntOp $0 $0 + ${ESTIMATEDSIZE}
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "$0"

  !if "${HOMEPAGE}" != ""
    WriteRegStr SHCTX "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
    WriteRegStr SHCTX "${UNINSTKEY}" "URLUpdateInfo" "${HOMEPAGE}"
    WriteRegStr SHCTX "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
  !endif

  ; Create start menu shortcut
  !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
    Call CreateOrUpdateStartMenuShortcut
  !insertmacro MUI_STARTMENU_WRITE_END

  ; Create desktop shortcut for silent and passive installers
  ; because finish page will be skipped
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    Call CreateOrUpdateDesktopShortcut
  ${EndIf}

  !ifmacrodef NSIS_HOOK_POSTINSTALL
    !insertmacro NSIS_HOOK_POSTINSTALL
  !endif

  ; The version record and owning layout are published only after the
  ; mandatory files have been successfully written.
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr SHCTX "${UNINSTKEY}" "RosenfuLocalLayout" "${ROSENFU_LOCAL_LAYOUT}"
  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
  Goto rosenfu_install_done
  rosenfu_payload_failed:
    StrCpy $R9 "必須ファイルを書き込めないため中止しました。新しい版として登録していません。"
    Call RosenfuRefuse
  rosenfu_install_done:
SectionEnd

Function .onInstSuccess
  ; Check for `/R` flag only in silent and passive installers because
  ; GUI installer has a toggle for the user to (re)start the app
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.onInit
  !insertmacro SetContext
  Call un.RosenfuRequireOwnedTarget

  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_UNINIT
  !endif

  !insertmacro MUI_UNGETLANGUAGE

  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}
FunctionEnd

; Compiled into the uninstaller; do not use an installer-only function.
; Treat _?=, a relocated uninstaller or stale registration as untrusted
; context. Never retarget the uninstaller by assigning $INSTDIR.
Function un.RosenfuRequireOwnedTarget
  StrCpy $0 "$INSTDIR" 8 -8
  StrCmp $0 "\rosenfu" 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "RosenfuLocalLayout"
  StrCmp $0 "${ROSENFU_LOCAL_LAYOUT}" 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "DisplayName"
  StrCmp $0 "${PRODUCTNAME}" 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${MANUPRODUCTKEY}" ""
  StrCmp $0 $INSTDIR 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "InstallLocation"
  StrCmp $0 '$\"$INSTDIR$\"' 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "UninstallString"
  StrCmp $0 '$\"$INSTDIR\uninstall.exe$\"' 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "MainBinaryName"
  StrCmp $0 "${MAINBINARYNAME}.exe" 0 un_rosenfu_invalid
  ReadRegStr $0 HKLM "${UNINSTKEY}" "DisplayVersion"
  StrCmp $0 "${VERSION}" 0 un_rosenfu_invalid

  ClearErrors
  GetFullPathName $7 "$INSTDIR"
  IfErrors un_rosenfu_invalid
  StrCmp $7 $INSTDIR 0 un_rosenfu_invalid
  StrCpy $6 "$INSTDIR" 3
  System::Call 'kernel32::GetDriveTypeW(w r6) i .r1'
  StrCmp $1 3 0 un_rosenfu_invalid
  StrCpy $0 "$INSTDIR"
  un_rosenfu_ancestor:
    StrCmp $0 "" un_rosenfu_invalid
    StrCpy $1 "$0\"
    StrCmp $1 $6 un_rosenfu_files
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 16 0 un_rosenfu_invalid
    ${GetParent} "$0" $3
    StrCmp $3 $0 un_rosenfu_invalid
    StrCpy $0 "$3"
    Goto un_rosenfu_ancestor
  un_rosenfu_files:
    ${If} $INSTDIR != "${ROSENFU_LOCAL_DIR}"
      StrCpy $RosenfuAclPath "$INSTDIR"
      Call un.RosenfuCheckCustomTree
      StrCmp $RosenfuAclOk 1 0 un_rosenfu_invalid
      Call un.RosenfuCheckInstalledAcl
      StrCmp $RosenfuAclOk 1 0 un_rosenfu_invalid
    ${EndIf}
    StrCpy $0 "$INSTDIR\${MAINBINARYNAME}.exe"
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 0 0 un_rosenfu_invalid
    StrCpy $0 "$INSTDIR\uninstall.exe"
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 0 0 un_rosenfu_invalid
    Return
  un_rosenfu_invalid:
    StrCpy $R9 "登録済みの管理者用 QA 版と一致しません。ファイルと登録は変更しません。"
    DetailPrint "$R9"
    IfSilent +2
      MessageBox MB_ICONSTOP|MB_OK "$R9"
    SetErrorLevel 2
    Abort
FunctionEnd

Section Uninstall
  Call un.RosenfuRequireOwnedTarget

  !ifmacrodef NSIS_HOOK_PREUNINSTALL
    !insertmacro NSIS_HOOK_PREUNINSTALL
  !endif

  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  ; Delete the app directory and its content from disk
  ; Copy main executable
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"

  ; Delete resources
  {{#each resources}}
    Delete "$INSTDIR\\{{this.[1]}}"
  {{/each}}

  ; Delete external binaries
  {{#each binaries}}
    Delete "$INSTDIR\\{{this}}"
  {{/each}}

  ; Delete app associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
      !insertmacro APP_UNASSOCIATE "{{ext}}" "{{or association.name ext}}"
    {{/each}}
  {{/each}}

  ; Delete deep links
  {{#each deep_link_protocols as |protocol| ~}}
    ReadRegStr $R7 SHCTX "Software\Classes\\{{protocol}}\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey SHCTX "Software\Classes\\{{protocol}}"
    ${EndIf}
  {{/each}}


  ; Delete uninstaller
  Delete "$INSTDIR\uninstall.exe"

  {{#each resources_ancestors}}
  RMDir /REBOOTOK "$INSTDIR\\{{this}}"
  {{/each}}
  RMDir "$INSTDIR"

  ; Remove shortcuts if not updating
  ${If} $UpdateMode <> 1
    !insertmacro DeleteAppUserModelId

    ; Remove start menu shortcut
    !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
    !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      RMDir "$SMPROGRAMS\$AppStartMenuFolder"
    ${EndIf}
    !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}

    ; Remove desktop shortcuts
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
      Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}

  ; Remove registry information for add/remove programs
  !if "${INSTALLMODE}" == "both"
    DeleteRegKey SHCTX "${UNINSTKEY}"
  !else if "${INSTALLMODE}" == "perMachine"
    DeleteRegKey HKLM "${UNINSTKEY}"
  !else
    DeleteRegKey HKCU "${UNINSTKEY}"
  !endif

  ; Machine uninstall never touches per-user saves. Remove only the owned
  ; machine install path record so a subsequent clean install defaults anew.
  DeleteRegValue HKLM "${MANUPRODUCTKEY}" ""
  DeleteRegKey /ifempty HKLM "${MANUPRODUCTKEY}"
  DeleteRegKey /ifempty HKLM "${MANUKEY}"

  !ifmacrodef NSIS_HOOK_POSTUNINSTALL
    !insertmacro NSIS_HOOK_POSTUNINSTALL
  !endif

  ; Auto close if passive mode or updating
  ${If} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Function RestorePreviousInstallLocation
  ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
  StrCmp $4 "" +2 0
    StrCpy $INSTDIR $4
FunctionEnd

Function Skip
  Abort
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd
Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd

Function CreateOrUpdateStartMenuShortcut
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  StrCpy $R0 0

  !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    StrCpy $R0 1
  ${EndIf}

  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    StrCpy $R0 1
  ${EndIf}

  ${If} $R0 = 1
    Return
  ${EndIf}

  ; Skip creating a shortcut in update/no-shortcut mode.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  !if "${STARTMENUFOLDER}" != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  !else
    CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  !endif
FunctionEnd

Function CreateOrUpdateDesktopShortcut
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Return
  ${EndIf}

  ; Skip creating a shortcut in update/no-shortcut mode.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
FunctionEnd
