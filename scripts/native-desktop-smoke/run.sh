#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
here="$PWD"
harness="$here/scripts/native-desktop-smoke"
python3 -B "$harness/guard.py"
python3 -B -m unittest discover -v -s "$harness"
qa=$(mktemp -d "${RUNNER_TEMP:?}/rosenfu-native-desktop.XXXXXX")
trap 'rm -rf -- "$qa"' EXIT
command -v WebKitWebDriver xvfb-run xauth dbus-run-session
pkg-config --exists webkit2gtk-4.1
export CARGO_TARGET_DIR="$qa/target" CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
npm ci
cargo install tauri-driver --version 2.1.0 --locked --root "$qa/tools"
driver="$qa/tools/bin/tauri-driver"
app="$CARGO_TARGET_DIR/debug/rosenfu-desktop"
run_screen() {
    local tag="$1"; shift
    if ! xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session -- "$@" --app "$app" \
           --driver "$driver" --logs "$qa/$tag"; then
        if [[ -f "$qa/$tag/driver.log" ]]; then tail -75 "$qa/$tag/driver.log" >&2; fi
        return 1
    fi
}
# Default Bus explicitly refuses a lab-only catalog.
npm run tauri -- build --debug --no-bundle
test -x "$app"
result=0
run_screen bus-seam python3 -B "$harness/smoke.py" --scenario seam || result=1
run_screen bus-loop python3 -B "$harness/smoke.py" --scenario loop || result=1
run_screen authored-tram python3 -B "$harness/smoke_authored_tram.py" || result=1
# Reuse the same target/dependency cache; rebuild the app with debug-only lab.
npm run tauri -- build --debug --no-bundle --features dev-road-lab
test -x "$app"
mkdir "$qa/native"
ln -s "$here" "$qa/exact"
cp -r "$harness/seed/." "$qa/native/"
cargo build --locked --manifest-path "$qa/native/Cargo.toml" --features dev-road-lab
run_screen tram python3 -B "$harness/smoke_tram.py" --seed "$CARGO_TARGET_DIR/debug/rosenfu-operation-view-check" || result=1
exit "$result"
