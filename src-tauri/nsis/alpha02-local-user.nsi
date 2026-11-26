; Rosenfu local-only current-user companion, minimal NSIS adaptation.
; Portions from Tauri CLI 2.12.4, 993038c49185307dfc895ab7657d77ca0d5ed7c7.
; Copyright (c) 2017 - Present Tauri Apps Contributors. See LICENSE-Tauri-MIT.
Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include MUI2.nsh
!include LogicLib.nsh
!include FileFunc.nsh
!include WordFunc.nsh

!define PRODUCT "{{product_name}}"
!define VERSION "{{version}}"
!define BUNDLEID "{{bundle_id}}"
!define APP "{{main_binary_name}}.exe"
!define QAPARENT "Software\Microsoft\Windows\CurrentVersion\Uninstall"
!define QAKEY "${QAPARENT}\${PRODUCT}"
!define PRIVATEQAKEY "${QAPARENT}\${BUNDLEID}"
!define MACHINEQAKEY "Software\{{manufacturer}}\${PRODUCT}"
!define MACHINEQADIR "$PROGRAMFILES64\rosenfu"
!define DISPLAY "${PRODUCT} (Current User)"
!define LAYOUT "rosenfu-local-user-v1"
!define PLACEHOLDER "placeholder\rosenfu"
!define WEBVIEW "Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!if "{{install_mode}}" != "currentUser"
  !error "Rosenfu user installer requires currentUser"
!endif
!if "{{arch}}" != "x64"
  !error "Rosenfu user installer requires x64"
!endif
!if "${PRODUCT}" != "Rosenfu Alpha 0.2 Test"
  !error "Apply the Windows QA overlay before the local-user overlay"
!endif
!if "${BUNDLEID}" != "io.github.mei8n.rosenfu.alpha02qa"
  !error "Apply the Windows QA overlay before the local-user overlay"
!endif
!if "{{install_webview2_mode}}" != ""
  !error "Local user setup requires the no-bootstrapper profile"
!endif
; This companion handles only the current standalone Rosenfu QA executable.
; Fail at build if a future profile needs extra payload handling.
{{#if resources}}
!error "Review packaging support before including resource files"
{{/if}}
{{#if binaries}}
!error "Review packaging support before including sidecars"
{{/if}}
{{#if file_associations}}
!error "Review packaging support before installing associations"
{{/if}}
{{#if deep_link_protocols}}
!error "Review packaging support before installing deep links"
{{/if}}
{{#if installer_hooks}}
!error "Local user setup does not load installer hooks"
{{/if}}

Name "${DISPLAY}"
OutFile "{{out_file}}"
InstallDir "${PLACEHOLDER}"
VIProductVersion "{{version_with_build}}"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "FileDescription" "${DISPLAY} Setup"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "{{copyright}}"
!addplugindir "{{additional_plugins_path}}"
!if "{{installer_icon}}" != ""
  !define MUI_ICON "{{installer_icon}}"
!endif

Var RegisteredDir
Var CanUsePath
Var Refusal
Var PayloadReservation
Var PayloadStage
!define MUI_PAGE_CUSTOMFUNCTION_PRE UserChooseDirectory
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE UserConfirmDirectory
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION UserLaunch
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}

; Requires an unelevated primary process token, independent of manifest and
; UAC settings. Query errors are denied and the opened token is always closed.
!macro RequireUserToken refusal
  System::Call 'kernel32::GetCurrentProcess() p .r0'
  System::Call 'advapi32::OpenProcessToken(p r0, i 8, *p .r1) i .r2'
  ${If} $2 = 0
    Call ${refusal}
  ${EndIf}
  ; TOKEN_INFORMATION_CLASS.TokenElevation=20, TOKEN_ELEVATION is one DWORD.
  System::Call 'advapi32::GetTokenInformation(p r1, i 20, *i .r3, i 4, *i .r4) i .r2'
  System::Call 'kernel32::CloseHandle(p r1)'
  ${If} $2 = 0
  ${OrIf} $4 != 4
  ${OrIf} $3 != 0
    Call ${refusal}
  ${EndIf}
!macroend

; Use only absolute local fixed-drive paths with the literal Rosenfu leaf.
; Do not regard this check as a race-resistant elevated writer.
!macro VerifyUserPath tag
  StrCpy $CanUsePath 0
  ; Reserved regardless of whether the machine install is fully registered.
  StrCmp $INSTDIR "${MACHINEQADIR}" ${tag}_done
  StrCpy $0 "$INSTDIR" 8 -8
  StrCmp $0 "\rosenfu" 0 ${tag}_done
  StrCpy $0 "$INSTDIR" 2 1
  StrCmp $0 ":\" 0 ${tag}_done
  StrCpy $6 "$INSTDIR" 3
  System::Call 'kernel32::GetDriveTypeW(w r6) i .r1'
  StrCmp $1 3 0 ${tag}_done
  ${GetParent} "$INSTDIR" $0
  ClearErrors
  GetFullPathName $7 "$0"
  IfErrors ${tag}_done
  StrCmp $7 $0 0 ${tag}_done
  ${tag}_ancestors:
    StrCmp $0 "" ${tag}_done
    StrCpy $1 "$0\"
    StrCmp $1 $6 ${tag}_leaf
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 16 0 ${tag}_done
    ${GetParent} "$0" $3
    StrCmp $3 $0 ${tag}_done
    StrCpy $0 "$3"
    Goto ${tag}_ancestors
  ${tag}_leaf:
    StrCpy $0 "$INSTDIR"
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1 ?e'
    Pop $2
    ${If} $RegisteredDir == ""
      ${If} $1 == -1
      ${AndIf} $2 == 2
        StrCpy $CanUsePath 1
      ${EndIf}
      Goto ${tag}_done
    ${EndIf}
    StrCmp $INSTDIR $RegisteredDir 0 ${tag}_done
    IntOp $2 $1 & 0x410
    StrCmp $2 16 0 ${tag}_done
    StrCpy $0 "$INSTDIR\${APP}"
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 0 0 ${tag}_done
    StrCpy $0 "$INSTDIR\uninstall.exe"
    System::Call 'kernel32::GetFileAttributesW(w r0) i .r1'
    IntOp $2 $1 & 0x410
    StrCmp $2 0 0 ${tag}_done
    StrCpy $CanUsePath 1
  ${tag}_done:
!macroend

!macro RefuseUserSetup
  DetailPrint "$Refusal"
  IfSilent +2
    MessageBox MB_ICONSTOP|MB_OK "$Refusal"
  SetErrorLevel 2
  Quit
!macroend

Function UserRefuse
  !insertmacro RefuseUserSetup
FunctionEnd

Function un.UserRefuse
  !insertmacro RefuseUserSetup
FunctionEnd

Function UserRequireToken
  StrCpy $Refusal "ユーザー用セットアップは管理者として起動しないでください。通常のエクスプローラーから開いてください。"
  !insertmacro RequireUserToken UserRefuse
FunctionEnd

Function un.UserRequireToken
  StrCpy $Refusal "ユーザー用アンインストーラーは管理者として起動しないでください。"
  !insertmacro RequireUserToken un.UserRefuse
FunctionEnd

!macro RequireAbsentRegistryKey hive key flags refusal
  ; Installed apps keys specifically for QA, not the main application.
  ; Only ERROR_FILE_NOT_FOUND permits install; no registry value is run.
  System::Call 'advapi32::RegOpenKeyExW(p ${hive}, w "${key}", i 0, i ${flags}, *p .r0) i .r1'
  ${If} $1 = 0
    System::Call 'advapi32::RegCloseKey(p r0)'
  ${EndIf}
  ${If} $1 != 2
    Call ${refusal}
  ${EndIf}
!macroend

!macro RequireNoForeignInstall refusal
  ; KEY_READ in both WOW64 views, with no writes to these keys.
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${QAKEY}" 0x20119 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${QAKEY}" 0x20219 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${PRIVATEQAKEY}" 0x20119 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${PRIVATEQAKEY}" 0x20219 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${MACHINEQAKEY}" 0x20119 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000002 "${MACHINEQAKEY}" 0x20219 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000001 "${PRIVATEQAKEY}" 0x20119 ${refusal}
  !insertmacro RequireAbsentRegistryKey 0x80000001 "${PRIVATEQAKEY}" 0x20219 ${refusal}
!macroend

Function UserRequireNoForeignInstall
  StrCpy $Refusal "PC 共通版または別の QA 版が登録されているか、確認できません。その版は変更しません。"
  !insertmacro RequireNoForeignInstall UserRefuse
FunctionEnd

Function un.UserRequireNoForeignInstall
  StrCpy $Refusal "PC 共通版または別の QA 版が登録されているか、確認できません。その版は変更しません。"
  !insertmacro RequireNoForeignInstall un.UserRefuse
FunctionEnd

Function UserReadRegistration
  StrCpy $RegisteredDir ""
  ClearErrors
  EnumRegValue $0 HKCU "${QAKEY}" 0
  IfErrors user_no_first_value
  Goto user_read
  user_no_first_value:
    ; An absent key is the only valid unowned state (deny empty/partial keys).
    System::Call 'advapi32::RegOpenKeyExW(p 0x80000001, w "${QAKEY}", i 0, i 0x20119, *p .r0) i .r1'
    ${If} $1 = 2
      Return
    ${EndIf}
    ${If} $1 = 0
      System::Call 'advapi32::RegCloseKey(p r0)'
    ${EndIf}
    Goto user_bad_registration
  user_read:
    ReadRegStr $0 HKCU "${QAKEY}" "RosenfuLocalLayout"
    StrCmp $0 "${LAYOUT}" 0 user_bad_registration
    ReadRegStr $0 HKCU "${QAKEY}" "DisplayName"
    StrCmp $0 "${DISPLAY}" 0 user_bad_registration
    ReadRegStr $RegisteredDir HKCU "${QAKEY}" "InstallLocation"
    StrCmp $RegisteredDir "" user_bad_registration
    ReadRegStr $0 HKCU "${QAKEY}" "UninstallString"
    StrCmp $0 '$\"$RegisteredDir\uninstall.exe$\"' 0 user_bad_registration
    ReadRegStr $0 HKCU "${QAKEY}" "DisplayVersion"
    StrCmp $0 "" user_bad_registration
    nsis_tauri_utils::SemverCompare "${VERSION}" "$0"
    Pop $1
    ${If} $1 = 0
    ${OrIf} $1 = 1
      Return
    ${EndIf}
  user_bad_registration:
    StrCpy $Refusal "以前の QA 版、別のインストール方式、または登録の不整合があります。自動で移行しません。元の QA 版を確認してください。"
    Call UserRefuse
FunctionEnd

Function .onInit
  Call UserRequireToken
  SetShellVarContext current
  SetRegView 64
  Call UserRequireNoForeignInstall
  StrCpy $Refusal "このセットアップの /D 指定は使えません。初回は画面でフォルダーを選んでください。"
  StrCmp $INSTDIR "${PLACEHOLDER}" +2
    Call UserRefuse
  Call UserReadRegistration
  ${If} $RegisteredDir != ""
    StrCpy $INSTDIR "$RegisteredDir"
  ${Else}
    StrCpy $INSTDIR "$LOCALAPPDATA\rosenfu"
  ${EndIf}
FunctionEnd

Function UserChooseDirectory
  ${If} $RegisteredDir != ""
    Abort ; MUI pre: hide the directory page for an in-place update.
  ${EndIf}
FunctionEnd

Function UserConfirmDirectory
  !insertmacro VerifyUserPath user_dialog
  ${If} $CanUsePath != 1
    MessageBox MB_ICONSTOP|MB_OK "書き込めるローカルドライブ上のフォルダーを選んでください。末尾を \rosenfu とし、まだ存在しない場所にしてください。"
    Abort
  ${EndIf}
FunctionEnd

Function UserRequireWebView
  StrCpy $Refusal "Microsoft Edge WebView2 Evergreen Runtime が必要です。Microsoft 公式ランタイムを導入してからセットアップをやり直してください。"
  ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\${WEBVIEW}" "pv"
  Call UserWebViewVersionOk
  ${If} $R0 != 1
    ReadRegStr $4 HKCU "Software\${WEBVIEW}" "pv"
    Call UserWebViewVersionOk
  ${EndIf}
  ${If} $R0 != 1
    Call UserRefuse
  ${EndIf}
FunctionEnd

Function UserWebViewVersionOk
  StrCmp $4 "" user_no_version
  ${VersionCompare} "$4" "0.0.0.0" $R0
  StrCmp $R0 1 0 user_no_version
  !if "{{minimum_webview2_version}}" != ""
    ${VersionCompare} "$4" "{{minimum_webview2_version}}" $R0
    StrCmp $R0 2 user_no_version
  !endif
  StrCpy $R0 1
  Return
  user_no_version:
    StrCpy $R0 0
FunctionEnd

Section Install
  Call UserRequireToken
  Call UserRequireNoForeignInstall
  Call UserReadRegistration
  !insertmacro VerifyUserPath user_install_path
  StrCpy $Refusal "指定フォルダーと登録済みの QA 版が一致しないか、指定先を使用できません。"
  ${If} $CanUsePath != 1
    Call UserRefuse
  ${EndIf}
  Call UserRequireWebView

  ; Create a unique staging directory on the chosen volume. NSIS File
  ; and WriteUninstaller never open installed names for writing.
  StrCpy $PayloadReservation ""
  StrCpy $PayloadStage ""
  ; Do not adopt a new leaf that appeared since validation. CreateDirectoryW
  ; must succeed on a fresh install; unlike SetOutPath it rejects existence.
  ${If} $RegisteredDir == ""
    System::Call 'kernel32::CreateDirectoryW(w "$INSTDIR", p 0) i .r1'
    StrCmp $1 0 user_write_failed
  ${EndIf}
  ClearErrors
  SetOutPath "$INSTDIR"
  IfErrors user_write_failed
  ClearErrors
  GetTempFileName $PayloadReservation "$INSTDIR"
  IfErrors user_write_failed
  StrCmp $PayloadReservation "" user_write_failed
  StrCpy $0 "$PayloadReservation.stage"
  System::Call 'kernel32::CreateDirectoryW(w r0, p 0) i .r1'
  StrCmp $1 0 user_write_failed
  ; Set only after CreateDirectoryW proved this directory is new.
  StrCpy $PayloadStage "$0"
  ClearErrors
  SetOutPath "$PayloadStage"
  IfErrors user_write_failed
  SetOverwrite on
  AllowSkipFiles off
  ClearErrors
  File "{{main_binary_path}}"
  IfErrors user_write_failed
  ClearErrors
  WriteUninstaller "$PayloadStage\uninstall.exe"
  IfErrors user_write_failed
  ClearErrors
  SetOutPath "$INSTDIR"
  IfErrors user_write_failed

  ; Rename directory entries, never truncate an existing destination file.
  ; Do not allow cross-volume copy or delayed/reboot behavior.
  StrCpy $3 0
  ${If} $RegisteredDir != ""
    StrCpy $3 1 ; MOVEFILE_REPLACE_EXISTING
  ${EndIf}
  StrCpy $0 "$PayloadStage\${APP}"
  StrCpy $1 "$INSTDIR\${APP}"
  System::Call 'kernel32::MoveFileExW(w r0, w r1, i r3) i .r2'
  StrCmp $2 0 user_write_failed
  StrCpy $0 "$PayloadStage\uninstall.exe"
  StrCpy $1 "$INSTDIR\uninstall.exe"
  System::Call 'kernel32::MoveFileExW(w r0, w r1, i r3) i .r2'
  StrCmp $2 0 user_write_failed
  Call UserCleanStage

  ; No app-data registration, bootstrapper, shell association or scope migration.
  ClearErrors
  WriteRegStr HKCU "${QAKEY}" "DisplayName" "${DISPLAY}"
  WriteRegStr HKCU "${QAKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${QAKEY}" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKCU "${QAKEY}" "DisplayIcon" '$\"$INSTDIR\${APP}$\"'
  WriteRegDWORD HKCU "${QAKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${QAKEY}" "NoRepair" 1
  IfErrors user_write_failed
  WriteRegStr HKCU "${QAKEY}" "DisplayVersion" "${VERSION}"
  IfErrors user_write_failed
  WriteRegStr HKCU "${QAKEY}" "RosenfuLocalLayout" "${LAYOUT}"
  IfErrors user_write_failed
  Goto user_install_done
  user_write_failed:
    Call UserCleanStage
    StrCpy $Refusal "必須のファイルまたは登録を書き込めません。セットアップを中止しました。"
    Call UserRefuse
  user_install_done:
SectionEnd

Function UserCleanStage
  ; Delete no installed names and do not recursively remove directories.
  ${If} $PayloadStage != ""
    System::Call 'kernel32::SetCurrentDirectoryW(w "$INSTDIR") i .r2'
    Delete "$PayloadStage\${APP}"
    Delete "$PayloadStage\uninstall.exe"
    RMDir "$PayloadStage"
    StrCpy $PayloadStage ""
  ${EndIf}
  ${If} $PayloadReservation != ""
    Delete "$PayloadReservation"
    StrCpy $PayloadReservation ""
  ${EndIf}
FunctionEnd

Function UserLaunch
  Call UserRequireToken
  Exec '"$INSTDIR\${APP}"'
FunctionEnd

Function un.onInit
  Call un.UserRequireToken
  SetShellVarContext current
  SetRegView 64
  Call un.UserRequireNoForeignInstall
  Call un.UserRequireOwnedPath
FunctionEnd

Function un.UserRequireOwnedPath
  StrCpy $Refusal "現在のユーザーが登録した QA 版と一致しません。ファイルや登録は削除しません。"
  ReadRegStr $0 HKCU "${QAKEY}" "RosenfuLocalLayout"
  StrCmp $0 "${LAYOUT}" 0 user_unowned
  ReadRegStr $0 HKCU "${QAKEY}" "DisplayName"
  StrCmp $0 "${DISPLAY}" 0 user_unowned
  ReadRegStr $RegisteredDir HKCU "${QAKEY}" "InstallLocation"
  StrCmp $RegisteredDir "" user_unowned
  ReadRegStr $0 HKCU "${QAKEY}" "UninstallString"
  StrCmp $0 '$\"$RegisteredDir\uninstall.exe$\"' 0 user_unowned
  ReadRegStr $0 HKCU "${QAKEY}" "DisplayVersion"
  StrCmp $0 "${VERSION}" 0 user_unowned
  !insertmacro VerifyUserPath user_uninstall
  StrCmp $CanUsePath 1 user_unowned_done
  user_unowned:
    Call un.UserRefuse
  user_unowned_done:
FunctionEnd

Section Uninstall
  Call un.UserRequireToken
  Call un.UserRequireNoForeignInstall
  Call un.UserRequireOwnedPath
  ClearErrors
  Delete "$INSTDIR\${APP}"
  IfErrors user_delete_failed
  Delete "$INSTDIR\uninstall.exe"
  IfErrors user_delete_failed
  DeleteRegKey HKCU "${QAKEY}"
  IfErrors user_delete_failed
  ; Do not remove unknown files from the chosen folder or any saved data.
  RMDir "$INSTDIR"
  Goto user_uninstall_done
  user_delete_failed:
    StrCpy $Refusal "必須のファイルまたは登録を削除できません。セーブデータには触れていません。"
    Call un.UserRefuse
  user_uninstall_done:
SectionEnd
