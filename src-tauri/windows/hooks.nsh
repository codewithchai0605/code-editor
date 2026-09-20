; Adds "Open with <App>" to a folder's right-click menu, and to the
; right-click menu inside a folder, during install; removes both during
; uninstall. This mirrors exactly what the in-app command "Preferences: Add
; 'Open With' to Folder Right-Click Menu" does — see the `windows` submodule
; in src-tauri/src/commands/native_integration.rs — so either one can be
; used interchangeably and either one fully undoes the other. Keep the two
; in sync if you ever change one.
;
; Everything here writes to HKEY_CURRENT_USER, never HKLM/HKCR directly:
; Explorer reads HKCU\Software\Classes with the same effect as
; HKEY_CLASSES_ROOT, and writing there never needs admin rights — whether
; this runs from an elevated installer or a per-user one.
;
; Wire this up in tauri.conf.json:
;
;   {
;     "bundle": {
;       "windows": {
;         "nsis": {
;           "installerHooks": "./windows/hooks.nsh"
;         }
;       }
;     }
;   }
;
; (path is relative to tauri.conf.json, i.e. src-tauri/)

!macro NSIS_HOOK_POSTINSTALL
  ; A real interactive installer *page* with a checkbox (closer to VS
  ; Code's own installer) needs a fully custom NSIS template — Tauri's
  ; installer-hooks mechanism only lets you run script at fixed points
  ; inside the existing install/uninstall flow, not add new wizard pages.
  ; A plain yes/no prompt is what that safely allows; see the project's
  ; notes on this trade-off for why a full custom template wasn't used
  ; instead.
  MessageBox MB_YESNO|MB_ICONQUESTION "Add '$(^Name)' to the folder right-click menu (Open with...)?" IDNO code_editor_skip_context_menu

    ; Right-click a folder itself — Explorer substitutes that folder's path
    ; for %1.
    WriteRegStr HKCU "Software\Classes\Directory\shell\CodeEditorOpenFolder" "" "Open with $(^Name)"
    WriteRegStr HKCU "Software\Classes\Directory\shell\CodeEditorOpenFolder" "Icon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
    WriteRegStr HKCU "Software\Classes\Directory\shell\CodeEditorOpenFolder\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'

    ; Right-click empty space inside a folder — Explorer substitutes the
    ; folder being viewed for %V.
    WriteRegStr HKCU "Software\Classes\Directory\Background\shell\CodeEditorOpenFolder" "" "Open with $(^Name)"
    WriteRegStr HKCU "Software\Classes\Directory\Background\shell\CodeEditorOpenFolder" "Icon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
    WriteRegStr HKCU "Software\Classes\Directory\Background\shell\CodeEditorOpenFolder\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%V"'

  code_editor_skip_context_menu:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Silent, unlike install — removing an app's own traces on uninstall is
  ; expected cleanup, not something that needs asking about again.
  DeleteRegKey HKCU "Software\Classes\Directory\shell\CodeEditorOpenFolder"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\CodeEditorOpenFolder"
!macroend
