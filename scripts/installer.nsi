Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
Name "EarControl"
OutFile "${ROOT}\dist\EarControl_${VERSION}_x64-setup.exe"
InstallDir "$LOCALAPPDATA\EarControl"
InstallDirRegKey HKCU "Software\EarControl" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
AllowSkipFiles off
Icon "${ROOT}\desktop\assets\icon.ico"
UninstallIcon "${ROOT}\desktop\assets\icon.ico"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "EarControl"
VIAddVersionKey "FileDescription" "EarControl installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "EarControl contributors"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${ROOT}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Function .onInit
    ${IfNot} ${RunningX64}
        MessageBox MB_OK|MB_ICONSTOP "EarControl requires Windows 10/11 x64."
        Abort
    ${EndIf}
FunctionEnd
Section "EarControl"
    SetShellVarContext current
    SetOutPath "$INSTDIR"
    ClearErrors
    File /oname=EarControl.exe "${ROOT}\target\release\earcontrol.exe"
    IfErrors 0 +2
        Abort "EarControl.exe could not be replaced. Close EarControl and retry."
    File "${ROOT}\LICENSE"
    File /oname=README.md "${ROOT}\docs\portable-readme.md"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateShortcut "$SMPROGRAMS\EarControl.lnk" "$INSTDIR\EarControl.exe"
    WriteRegStr HKCU "Software\EarControl" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "DisplayName" "EarControl"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "DisplayIcon" "$INSTDIR\EarControl.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl" "NoRepair" 1
SectionEnd
Section "Uninstall"
    SetShellVarContext current
    retry_remove:
    ClearErrors
    Delete "$INSTDIR\EarControl.exe"
    IfErrors 0 removed_executable
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Close EarControl before uninstalling, then choose Retry." IDRETRY retry_remove
    Abort "EarControl.exe could not be removed. Installation registration was preserved."
    removed_executable:
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    Delete "$SMPROGRAMS\EarControl.lnk"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\EarControl"
    DeleteRegKey HKCU "Software\EarControl"
SectionEnd
