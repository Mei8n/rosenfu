# VS Code (Windows)

Open the repository root in VS Code, not just `src-tauri`.

## Required tools

Use the Node.js, Rust MSVC, Windows C++ build tools, and WebView2 prerequisites
described in [development.md](development.md).
Install the recommended **C/C++** extension (`ms-vscode.cpptools`) for the
Windows debugger. **rust-analyzer** provides Rust editing and diagnostics.
The recommendations do not install extensions automatically.

## F5: build and debug Rust

1. Set a breakpoint in a Rust command, for example `engine_info` in
   `src-tauri/src/main.rs`.
2. Select **Rosenfu: debug desktop (Windows)** in Run and Debug, then press F5.
3. The pre-launch task builds the frontend and a debug desktop executable.
4. Trigger the command in the app. Inspect variables, step, or continue with F5.

This configuration embeds the built frontend. It does **not** start Vite or
use port 1420, avoiding conflicts with a separately running development server.
It does not hot-reload: restart debugging to rebuild frontend or Rust changes.
The first debug build can take several minutes. Do not select "Debug Anyway"
if the build fails.

## Hot reload and attach

Run **Tasks: Run Task > Rosenfu: dev (hot reload)**.
After the app opens, select **Rosenfu: attach to running desktop (Windows)**,
press F5, and choose `rosenfu-desktop.exe`.

Rust edits can restart the process, so reattach after a restart. Stop the dev
task with Ctrl+C in its terminal when finished; disconnecting the debugger
does not stop the dev server. Avoid running multiple dev tasks at once.
If port 1420 is already in use, stop your earlier dev task first rather than
terminating an unidentified process.

## React / TypeScript debugging

In a debug desktop app, open WebView developer tools with Ctrl+Shift+I
(or Inspect from the context menu when available).
Use Console and Elements for frontend diagnostics. The F5 configuration
debugs Rust, not React. For original-source debugging with Vite's development
output, use the hot-reload workflow above.

## Build and tests

- Ctrl+Shift+B: **Rosenfu: build release desktop**.
  Output: `target/release/rosenfu-desktop.exe`.
- **Tasks: Run Test Task > Rosenfu: test all**: frontend tests, then all Rust
  workspace tests.
- **Tasks: Run Task > Rosenfu: check Rust formatting**: formatting check.
- **Rosenfu: build debug desktop**: build without starting a debugger.
  Output: `target/debug/rosenfu-desktop.exe`.

These tasks do not commit, publish, package an installer, or change global
VS Code settings. Debugger configurations are Windows-specific; the project
itself continues to support its existing OS build workflows.
