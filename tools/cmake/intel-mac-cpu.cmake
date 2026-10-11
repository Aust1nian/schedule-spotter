# Intel Macs run llama.cpp on the CPU (spec: "Metal on Apple Silicon, CPU on
# Intel Macs"). llama.cpp turns Metal on for every Apple build, and on the
# Intel runner it crashed (GGML_ASSERT(buf_dst)) after 62 s compiling
# shaders. Used only for x86_64-apple-darwin, via
# CMAKE_TOOLCHAIN_FILE_x86_64_apple_darwin in .cargo/config.toml.
set(GGML_METAL OFF CACHE BOOL "CPU on Intel Macs" FORCE)
