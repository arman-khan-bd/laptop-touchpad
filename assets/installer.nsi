; ============================================================================
; Laptop Touchpad Gesture Manager & Remapper
; NSIS Step-by-Step Modern Windows Installer Script
; ============================================================================

!include "MUI2.nsh"
!include "FileFunc.nsh"

; General Configuration
Name "Touchpad Gesture Manager"
OutFile "..\dist\gesture-manager-setup.exe"
InstallDir "$LOCALAPPDATA\GestureManager"
InstallDirRegKey HKCU "Software\GestureManager" "InstallDir"
RequestExecutionLevel user ; Runs without UAC elevation prompt

; Modern UI Appearance
!define MUI_ABORTWARNING
!ifFileExists "icon.ico"
  !define MUI_ICON "icon.ico"
  !define MUI_UNICON "icon.ico"
!endif

; Installer Pages (Step-by-Step Wizard)
; Step 1: Welcome Page
!insertmacro MUI_PAGE_WELCOME

; Step 2: License Agreement
!insertmacro MUI_PAGE_LICENSE "..\README.md"

; Step 3: Component Selection
!insertmacro MUI_PAGE_COMPONENTS

; Step 4: Installation Directory Selection
!insertmacro MUI_PAGE_DIRECTORY

; Step 5: File Installation Progress
!insertmacro MUI_PAGE_INSTFILES

; Step 6: Finish Page with Launch Option
!define MUI_FINISHPAGE_RUN "$INSTDIR\gesture-manager.exe"
!define MUI_FINISHPAGE_RUN_PARAMETERS "--gui"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Touchpad Gesture Manager & Settings GUI"
!insertmacro MUI_PAGE_FINISH

; Uninstaller Pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; ----------------------------------------------------------------------------
; Component Sections
; ----------------------------------------------------------------------------

Section "Core Daemon & Executable (Required)" SecCore
  SectionIn RO
  SetOutPath "$INSTDIR"
  
  File "..\dist\gesture-manager.exe"
  File "..\README.md"
  
  ; Write Uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"
  
  ; Registry Add/Remove Programs Registration
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager" \
                   "DisplayName" "Touchpad Gesture Manager"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager" \
                   "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager" \
                   "DisplayIcon" "$INSTDIR\gesture-manager.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager" \
                   "Publisher" "Gesture Manager Team"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager" \
                   "DisplayVersion" "0.1.0"
SectionEnd

Section "Start Menu & Desktop Shortcuts" SecShortcuts
  CreateDirectory "$SMPROGRAMS\Touchpad Gesture Manager"
  CreateShortcut "$SMPROGRAMS\Touchpad Gesture Manager\Gesture Manager.lnk" "$INSTDIR\gesture-manager.exe" "" "$INSTDIR\gesture-manager.exe" 0
  CreateShortcut "$SMPROGRAMS\Touchpad Gesture Manager\Gesture Configurator GUI.lnk" "$INSTDIR\gesture-manager.exe" "--gui" "$INSTDIR\gesture-manager.exe" 0
  CreateShortcut "$SMPROGRAMS\Touchpad Gesture Manager\Uninstall.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
  CreateShortcut "$DESKTOP\Gesture Manager.lnk" "$INSTDIR\gesture-manager.exe" "--gui" "$INSTDIR\gesture-manager.exe" 0
SectionEnd

Section "Start Automatically on Windows Login" SecAutoStart
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" \
                   "TouchpadGestureManager" "$\"$INSTDIR\gesture-manager.exe$\""
SectionEnd

; Component Descriptions
LangString DESC_SecCore ${LANG_ENGLISH} "The background touchpad raw input interception daemon and embedded GUI."
LangString DESC_SecShortcuts ${LANG_ENGLISH} "Create convenient shortcuts on your Desktop and in the Windows Start Menu."
LangString DESC_SecAutoStart ${LANG_ENGLISH} "Automatically launch the gesture daemon minimized in the system tray when Windows boots."

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} $(DESC_SecCore)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecShortcuts} $(DESC_SecShortcuts)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutoStart} $(DESC_SecAutoStart)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; ----------------------------------------------------------------------------
; Uninstallation Logic
; ----------------------------------------------------------------------------

Section "Uninstall"
  ; Stop running process if active
  ExecWait 'taskkill /F /IM gesture-manager.exe'
  
  Delete "$INSTDIR\gesture-manager.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  
  ; Remove Shortcuts
  Delete "$SMPROGRAMS\Touchpad Gesture Manager\Gesture Manager.lnk"
  Delete "$SMPROGRAMS\Touchpad Gesture Manager\Gesture Configurator GUI.lnk"
  Delete "$SMPROGRAMS\Touchpad Gesture Manager\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Touchpad Gesture Manager"
  Delete "$DESKTOP\Gesture Manager.lnk"
  
  ; Remove Registry Keys
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\GestureManager"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "TouchpadGestureManager"
SectionEnd
