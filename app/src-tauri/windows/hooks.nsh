; llama.cpp's DLLs, collected by tools/ci/collect-llama-libs.py after the
; build: the exe links llama.dll and ggml*.dll, and picks the best
; ggml-cpu-*.dll for the machine at start (DECISIONS.md, Phase 4).
!define SPOTTER_LLAMA_LIBS "${__FILEDIR__}\llama-libs"

!macro NSIS_HOOK_POSTINSTALL
  SetOutPath "$INSTDIR"
  File "${SPOTTER_LLAMA_LIBS}\*.dll"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$INSTDIR\llama*.dll"
  Delete "$INSTDIR\ggml*.dll"
!macroend
