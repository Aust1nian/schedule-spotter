; llama.cpp's DLLs, collected by tools/ci/collect-llama-libs.py after the
; build: the exe links llama.dll and ggml*.dll, and picks the best
; ggml-cpu-*.dll for the machine at start (DECISIONS.md, Phase 4).
!define SPOTTER_LLAMA_LIBS "${__FILEDIR__}\llama-libs"

; The model rides after the installer's own data (NSIS holds at most about
; 2 GB; tools/release/append-model.py). The app copies it into
; $INSTDIR\model and checks its SHA-256; a build with no model returns at
; once. On failure the app still installs and runs with smart matching off
; (spec: "Never fails because of the model"), and a silent install exits 2.
!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File "${SPOTTER_LLAMA_LIBS}\*.dll"
  DetailPrint "Installing smart matching (this can take a minute)..."
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --install-model "$EXEPATH"' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "Schedule Spotter installed, but its smart-matching file was damaged in the download, so it will ask you more questions. To fix this, download Schedule Spotter again and reinstall." /SD IDOK
    SetErrorLevel 2
  ${EndIf}
!macroend

; Tauri's uninstaller keeps its registry key (install folder, language)
; unless "delete app data" is ticked, which a silent uninstall never is. The
; app keeps nothing there, so it goes, except during an update.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegKey HKCU "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty HKCU "${MANUKEY}"
  ${EndIf}
  Delete "$INSTDIR\llama*.dll"
  Delete "$INSTDIR\ggml*.dll"
  RMDir /r "$INSTDIR\model"
  RMDir /r "$INSTDIR\Licenses"
  RMDir "$INSTDIR"
!macroend
