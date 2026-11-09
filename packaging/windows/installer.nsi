Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include MUI2.nsh
!include LogicLib.nsh
!include x64.nsh

!define PRODUCT "{{product_name}}"
!define VERSION "{{version}}"
!define APPKEY "Software\{{manufacturer}}\${PRODUCT}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"

Name "${PRODUCT}"
OutFile "{{out_file}}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT}"
VIProductVersion "{{version_with_build}}"
VIAddVersionKey "ProductName" "${PRODUCT}"
VIAddVersionKey "FileDescription" "{{short_description}}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 Suraj Mandal"
!define MUI_ICON "{{installer_icon}}"
!define MUI_UNICON "{{installer_icon}}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\hushpen.exe"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${If} $INSTDIR == "$LOCALAPPDATA\Programs\${PRODUCT}"
    ReadRegStr $0 HKCU "${APPKEY}" ""
    ${If} $0 != ""
      StrCpy $INSTDIR $0
    ${EndIf}
  ${EndIf}
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Hushpen requires 64-bit Windows."
    SetErrorLevel 1
    Abort
  ${EndIf}
FunctionEnd

!macro CheckApp
  nsis_tauri_utils::FindProcess "hushpen.exe"
  Pop $0
  ${If} $0 = 0
    IfSilent +2
    MessageBox MB_ICONSTOP "Close Hushpen before you continue."
    SetErrorLevel 2
    Abort
  ${EndIf}
!macroend

Section Install
  !insertmacro CheckApp
  SetOverwrite on
  ClearErrors
  SetOutPath "$INSTDIR"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort
  ${EndIf}
  File "{{main_binary_path}}"
  {{#each resources_dirs}}
  CreateDirectory "$INSTDIR\{{this}}"
  {{/each}}
  {{#each resources}}
  File /a "/oname={{this}}" "{{@key}}"
  {{/each}}
  {{#each binaries}}
  File /a "/oname={{this}}" "{{@key}}"
  {{/each}}
  ${If} ${Errors}
    SetErrorLevel 1
    Abort
  ${EndIf}
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateDirectory "$SMPROGRAMS\${PRODUCT}"
  CreateShortcut "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk" "$INSTDIR\hushpen.exe"
  WriteRegStr HKCU "${APPKEY}" "" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "{{manufacturer}}"
  WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\hushpen.exe"
  WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '$"$INSTDIR\uninstall.exe$"'
  WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '$"$INSTDIR\uninstall.exe$" /S'
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
  ${If} ${Errors}
    SetErrorLevel 1
    Abort
  ${EndIf}
SectionEnd

Section Uninstall
  SetShellVarContext current
  SetRegView 64
  !insertmacro CheckApp
  ClearErrors
  Delete "$INSTDIR\hushpen.exe"
  {{#each resources}}
  Delete "$INSTDIR\{{this}}"
  {{/each}}
  {{#each binaries}}
  Delete "$INSTDIR\{{this}}"
  {{/each}}
  Delete "$INSTDIR\uninstall.exe"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort
  ${EndIf}
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT}"
  DeleteRegKey HKCU "${UNINSTKEY}"
  DeleteRegKey HKCU "${APPKEY}"
SectionEnd
