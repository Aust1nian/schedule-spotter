import { defineConfig } from "vite";

// Safari 14 is the WKWebView floor on macOS 11; WebView2 is evergreen Chromium.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: ["es2021", "safari14"],
    outDir: "dist",
    assetsInlineLimit: 0,
    sourcemap: false,
  },
});
