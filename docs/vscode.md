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
3. The pre-launch task synchronizes dependencies, incrementally builds Rust,
   and starts Vite on port 1420 before launching the native debugger.
4. Trigger the command in the app. Inspect variables, step, or continue with F5.

The normal configuration uses the development server. Frontend changes hot-reload;
Rust changes require stopping and starting debugging again. Debugger stop also stops
the workspace-owned Vite server. If launching fails or the IDE crashes, run
**Rosenfu: stop debug UI**; the next F5 also performs this cleanup first.
Other Node processes are never terminated. Do not debug the same checkout twice
at once. A separately running server on port 1420 must be stopped by its owner.

Select **Rosenfu: debug embedded desktop (Windows)** to use the previous built-in
frontend workflow, with no server or hot reload. Both retain Rust debug symbols.
Initial native builds and Rust relinking can still take minutes; this change does
not guarantee a particular startup time. Do not select "Debug Anyway" if a task fails.

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
output, use normal F5 or the hot-reload workflow above.

## Build and tests

- Ctrl+Shift+B: **Rosenfu: build release desktop**.
  Output: `target/release/rosenfu-desktop.exe`.
- **Tasks: Run Test Task > Rosenfu: test all**: frontend tests, then all Rust
  workspace tests.
- **Tasks: Run Task > Rosenfu: check Rust formatting**: formatting check.
- **Rosenfu: build debug desktop**: build without starting a debugger.
  Output: `target/debug/rosenfu-desktop.exe`.

These tasks do not commit, publish, package an installer, or change global
VS Code settings. Windows uses cppvsdbg; macOS/Linux use CodeLLDB. Native GUI breakpoint tests
are separate from the three-OS CI build checks.
