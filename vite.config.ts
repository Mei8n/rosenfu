import { defineConfig } from "vite";

export default defineConfig({
  server: {
    watch: {
      // Rust outputs and native build tools must not enter Vite's file watcher.
      ignored: ["**/target/**", "**/src-tauri/**"],
    },
  },
});
