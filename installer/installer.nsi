; installer.nsi - Windows installer of Planche-Contact
;
; Built by build-installer.sh (on Linux: NSIS runs there too), which passes
; with -D on the makensis command line:
;   VERSION   version to display (e.g. 5.3.0)
;   DIST      folder holding everything to install: the published WinUI
;             window, engine\planche-contact-engine.exe,
;             planche-contact-verb.exe and docs\
;   OUTFILE   full path of the installer .exe to generate
;   ICON      path to the .ico icon
;
; The installation is per user: everything goes under the user's own
; folders and registry (HKEY_CURRENT_USER), so no administrator rights are
; asked for.
;
; The installer speaks the four languages of the program. NSIS picks that of
; Windows; English, declared first, is used when Windows is in another
; language.
;
; This file is UTF-8 with a byte order mark (BOM), which NSIS requires to
; read accents; do not save it without a BOM.

Unicode true

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef DIST
  !define DIST "..\dist\planche-contact"
!endif
!ifndef OUTFILE
  !define OUTFILE "..\dist\planche-contact-${VERSION}-setup.exe"
!endif
!ifndef ICON
  !define ICON "..\app\PlancheContact\Assets\AppIcon.ico"
!endif

!define PRODUCT "Planche-Contact"
!define APP_EXE "PlancheContact.exe"
!define ENGINE_EXE "engine\planche-contact-engine.exe"
!define VERB_EXE "planche-contact-verb.exe"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT}"
; The earlier version for Windows (GTK window, installed for all users).
!define OLD_UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\PlancheContact"
!define PROJECT_URL "https://github.com/gillesmagneville/planche-contact-windows"
; Windows PowerShell, 64-bit (the installer is a 32-bit program: "sysnative"
; reaches the real System32), for menu-package.ps1.
!define POWERSHELL "$WINDIR\sysnative\WindowsPowerShell\v1.0\powershell.exe"

Name "${PRODUCT}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT}"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"   ; ${GetSize}, ${DirState}
!include "WinVer.nsh"     ; ${AtLeastBuild}
!include "x64.nsh"        ; ${RunningX64}

!define MUI_ABORTWARNING
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"

; Last page: how to reach the program, and a box to start it now.
!define MUI_FINISHPAGE_TEXT "$(FINISH_TEXT)"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; English first: it is the fallback language.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "Spanish"
!insertmacro MUI_LANGUAGE "German"

; ---------------------------------------------------------------- texts

; The quoted menu entry must be the text the handler registers for the same
; language (shell-verb/src/labels.rs - its tests check this file).
LangString FINISH_TEXT ${LANG_ENGLISH} "${PRODUCT} is installed.$\r$\n$\r$\nStart it from the Start menu, or in File Explorer: select photos or a folder, right-click, then choose “Open in Planche-Contact”."
LangString FINISH_TEXT ${LANG_FRENCH}  "${PRODUCT} est installé.$\r$\n$\r$\nLancez-le depuis le menu Démarrer, ou dans l'Explorateur de fichiers : sélectionnez des photos ou un dossier, faites un clic droit, puis choisissez « Ouvrir dans Planche-Contact »."
LangString FINISH_TEXT ${LANG_SPANISH} "${PRODUCT} está instalado.$\r$\n$\r$\nInícielo desde el menú Inicio, o en el Explorador de archivos: seleccione fotos o una carpeta, haga clic con el botón derecho y elija «Abrir en Planche-Contact»."
LangString FINISH_TEXT ${LANG_GERMAN}  "${PRODUCT} ist installiert.$\r$\n$\r$\nStarten Sie es über das Startmenü oder im Datei-Explorer: Wählen Sie Fotos oder einen Ordner aus, klicken Sie mit der rechten Maustaste und wählen Sie „In Planche-Contact öffnen“."

LangString ERR_WINDOWS ${LANG_ENGLISH} "${PRODUCT} needs a 64-bit Windows 10 (version 1809 or later) or Windows 11."
LangString ERR_WINDOWS ${LANG_FRENCH}  "${PRODUCT} nécessite Windows 10 (version 1809 ou ultérieure) ou Windows 11, en 64 bits."
LangString ERR_WINDOWS ${LANG_SPANISH} "${PRODUCT} necesita Windows 10 (versión 1809 o posterior) o Windows 11, de 64 bits."
LangString ERR_WINDOWS ${LANG_GERMAN}  "${PRODUCT} benötigt ein 64-Bit-Windows 10 (Version 1809 oder neuer) oder Windows 11."

LangString WARN_OLD ${LANG_ENGLISH} "An earlier version of ${PRODUCT} is installed for all users. Uninstall it first (Settings > Apps > Installed apps), otherwise the File Explorer menu will show two entries.$\r$\n$\r$\nContinue anyway?"
LangString WARN_OLD ${LANG_FRENCH}  "Une version précédente de ${PRODUCT} est installée pour tous les utilisateurs. Désinstallez-la d'abord (Paramètres > Applications > Applications installées), sinon le menu de l'Explorateur de fichiers affichera deux entrées.$\r$\n$\r$\nContinuer quand même ?"
LangString WARN_OLD ${LANG_SPANISH} "Hay una versión anterior de ${PRODUCT} instalada para todos los usuarios. Desinstálela primero (Configuración > Aplicaciones > Aplicaciones instaladas); de lo contrario, el menú del Explorador de archivos mostrará dos entradas.$\r$\n$\r$\n¿Continuar de todos modos?"
LangString WARN_OLD ${LANG_GERMAN}  "Eine frühere Version von ${PRODUCT} ist für alle Benutzer installiert. Deinstallieren Sie sie zuerst (Einstellungen > Apps > Installierte Apps), sonst zeigt das Menü des Datei-Explorers zwei Einträge.$\r$\n$\r$\nTrotzdem fortfahren?"

LangString ERR_FOLDER ${LANG_ENGLISH} "The folder $INSTDIR already contains other files. Choose an empty folder: it will be entirely deleted when the program is uninstalled."
LangString ERR_FOLDER ${LANG_FRENCH}  "Le dossier $INSTDIR contient déjà d'autres fichiers. Choisissez un dossier vide : il sera entièrement supprimé à la désinstallation."
LangString ERR_FOLDER ${LANG_SPANISH} "La carpeta $INSTDIR ya contiene otros archivos. Elija una carpeta vacía: se eliminará por completo al desinstalar el programa."
LangString ERR_FOLDER ${LANG_GERMAN}  "Der Ordner $INSTDIR enthält bereits andere Dateien. Wählen Sie einen leeren Ordner: Er wird bei der Deinstallation vollständig gelöscht."

LangString ERR_RUNNING ${LANG_ENGLISH} "${PRODUCT} is running. Close its window, then try again."
LangString ERR_RUNNING ${LANG_FRENCH}  "${PRODUCT} est en cours d'exécution. Fermez sa fenêtre, puis réessayez."
LangString ERR_RUNNING ${LANG_SPANISH} "${PRODUCT} se está ejecutando. Cierre su ventana y vuelva a intentarlo."
LangString ERR_RUNNING ${LANG_GERMAN}  "${PRODUCT} wird gerade ausgeführt. Schließen Sie das Fenster und versuchen Sie es erneut."

LangString ERR_MENU ${LANG_ENGLISH} "The program is installed, but its entry could not be added to the File Explorer menu. Details: planche-contact-verb.log, in your temporary folder (%TEMP%)."
LangString ERR_MENU ${LANG_FRENCH}  "Le programme est installé, mais son entrée n'a pas pu être ajoutée au menu de l'Explorateur de fichiers. Détails : planche-contact-verb.log, dans votre dossier temporaire (%TEMP%)."
LangString ERR_MENU ${LANG_SPANISH} "El programa está instalado, pero no se pudo añadir su entrada al menú del Explorador de archivos. Detalles: planche-contact-verb.log, en su carpeta temporal (%TEMP%)."
LangString ERR_MENU ${LANG_GERMAN}  "Das Programm ist installiert, aber sein Eintrag konnte nicht zum Menü des Datei-Explorers hinzugefügt werden. Details: planche-contact-verb.log in Ihrem temporären Ordner (%TEMP%)."

VIProductVersion "${VERSION}.0"
!macro VersionKeys LANG DESCRIPTION
    VIAddVersionKey /LANG=${LANG} "ProductName" "${PRODUCT}"
    VIAddVersionKey /LANG=${LANG} "ProductVersion" "${VERSION}"
    VIAddVersionKey /LANG=${LANG} "CompanyName" "Gilles MAGNEVILLE"
    VIAddVersionKey /LANG=${LANG} "LegalCopyright" "GNU GPL v3"
    VIAddVersionKey /LANG=${LANG} "FileDescription" "${DESCRIPTION}"
    VIAddVersionKey /LANG=${LANG} "FileVersion" "${VERSION}"
!macroend
!insertmacro VersionKeys ${LANG_ENGLISH} "${PRODUCT} installer"
!insertmacro VersionKeys ${LANG_FRENCH}  "Installeur de ${PRODUCT}"
!insertmacro VersionKeys ${LANG_SPANISH} "Instalador de ${PRODUCT}"
!insertmacro VersionKeys ${LANG_GERMAN}  "Installationsprogramm für ${PRODUCT}"

; ------------------------------------------------------------- helpers

; Leaves 1 in $R0 if FILE exists and is in use (it cannot be opened for
; writing while its program is running), 0 otherwise.
!macro CheckInUse FILE
    ${If} ${FileExists} "${FILE}"
        ClearErrors
        FileOpen $R1 "${FILE}" a
        ${If} ${Errors}
            StrCpy $R0 1
        ${Else}
            FileClose $R1
        ${EndIf}
    ${EndIf}
!macroend

Function .onInit
    ${IfNot} ${RunningX64}
    ${OrIfNot} ${AtLeastBuild} 17763
        MessageBox MB_OK|MB_ICONSTOP "$(ERR_WINDOWS)" /SD IDOK
        Abort
    ${EndIf}

    ; The earlier version (GTK window) has its own menu entry and its own
    ; uninstaller, which needs administrator rights: only the user can
    ; remove it. Say so before going on.
    ClearErrors
    ReadRegStr $0 HKLM "${OLD_UNINSTALL_KEY}" "DisplayName"
    ${IfNot} ${Errors}
        MessageBox MB_OKCANCEL|MB_ICONEXCLAMATION "$(WARN_OLD)" /SD IDOK IDOK +2
        Abort
    ${EndIf}
FunctionEnd

; ------------------------------------------------------------- install

Section "Install"
    SetShellVarContext current

    ; The installation folder is deleted as a whole by an upgrade and by the
    ; uninstaller: never accept a folder that holds anything but a previous
    ; installation of ours. (The folder is not asked on screen; this guards
    ; the /D= option of the command line.)
    ${IfNot} ${FileExists} "$INSTDIR\${VERB_EXE}"
        ${DirState} "$INSTDIR" $0
        ${If} $0 == 1
            MessageBox MB_OK|MB_ICONSTOP "$(ERR_FOLDER)" /SD IDOK
            Abort
        ${EndIf}
    ${EndIf}

    ; Upgrade: the programs of the previous version must not be running.
    check_running:
        StrCpy $R0 0
        !insertmacro CheckInUse "$INSTDIR\${APP_EXE}"
        !insertmacro CheckInUse "$INSTDIR\${ENGINE_EXE}"
        ${If} $R0 == 1
            MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(ERR_RUNNING)" /SD IDCANCEL IDRETRY check_running
            Abort
        ${EndIf}

    ; Upgrade: remove the menu entries with the handler that created them
    ; (the list of file types may differ between versions), then the files
    ; of the previous version (none must linger among those of the new one).
    ${If} ${FileExists} "$INSTDIR\${VERB_EXE}"
        ${If} ${FileExists} "$INSTDIR\menu-package.ps1"
            nsExec::Exec '"${POWERSHELL}" -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File "$INSTDIR\menu-package.ps1" -Unregister'
            Pop $0
        ${EndIf}
        ExecWait '"$INSTDIR\${VERB_EXE}" --unregister'
        RMDir /r "$INSTDIR"
    ${EndIf}

    SetOutPath "$INSTDIR"
    File /r "${DIST}\*.*"
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Entry in Settings > Apps > Installed apps.
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Gilles MAGNEVILLE"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "${PROJECT_URL}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1

    ; Installed size, in kilobytes, shown in the "Size" column. Computed after
    ; all the files have been copied. $0 is a plain decimal number, which
    ; WriteRegDWORD takes as it is.
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"

    ; "Open in Planche-Contact" in the File Explorer menu. On Windows 11, on
    ; the first level of the menu: a sparse package (menu-package.ps1 says 0
    ; when it is registered, 2 on Windows 10). Otherwise, or if that fails,
    ; the classic entry ("Show more options" on Windows 11) - never both,
    ; which would show the entry twice under "Show more options". File
    ; Explorer is restarted after an interactive installation, so that the
    ; new entry shows at once.
    StrCpy $1 " -RestartExplorer"
    ${If} ${Silent}
        StrCpy $1 ""
    ${EndIf}
    nsExec::Exec '"${POWERSHELL}" -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File "$INSTDIR\menu-package.ps1" -Register$1'
    Pop $2
    ${If} $2 != 0
        ExecWait '"$INSTDIR\${VERB_EXE}" --register' $0
        ${If} $0 != 0
            MessageBox MB_OK|MB_ICONEXCLAMATION "$(ERR_MENU)" /SD IDOK
        ${EndIf}
    ${EndIf}

    ; Start Menu shortcut.
    CreateShortCut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\${APP_EXE}"
SectionEnd

; ----------------------------------------------------------- uninstall

Section "Uninstall"
    SetShellVarContext current

    ; First the menu entries, while what removes them is there.
    nsExec::Exec '"${POWERSHELL}" -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File "$INSTDIR\menu-package.ps1" -Unregister'
    Pop $0
    ExecWait '"$INSTDIR\${VERB_EXE}" --unregister'

    Delete "$SMPROGRAMS\${PRODUCT}.lnk"
    RMDir /r "$INSTDIR"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
    ; The settings of the window (%USERPROFILE%\.config\planche-contact) are
    ; left in place: they are shared with the earlier version and hold only
    ; folders and the language.
SectionEnd
