# One-off native desktop smoke

Scope: approved product revision `83fe184bc962861d11d3b65ab955646da1c65a4a` and **one child commit containing only this reviewed smoke change**, not additional product code. On this exact same-repository open draft #285, deliberately add `qa:native-desktop-83fe184bc962` **once**, after the smoke change itself is reviewed. Normal synchronize, pushes, ready-for-review, package jobs and rerun attempts do not execute this job. Relabeling is a new deliberate request and is not automatic. No new branch is needed.

The job runs ordinary default Bus seam and circuit scenarios in separate disposable profiles, using deterministic synthetic tile response bodies (not mocked Tauri commands). It also draws a normal Tram through a synthetic observed T intersection, checks computed green guide/preview/marker and the temporary free preview with Ctrl held and no mouse move, and checks native construction without assigning operational road authority, then places two 40m stations on that **same** built Tram, creates its normal service, and verifies the active train and shared clock across save/reopen, and runs the separately labeled opt-in Dejima lab Tram creation/edit and through-service fixture on the same Ubuntu runner with a debug-only no-bundle binary. The Tram service's approach tracks and platforms are synthetic. This validates native GUI interactions, execution, and disk reopen; it is not live CDN, actual legal map coverage, performance certification, or Windows/macOS verification. No installer, uploads, secret use, or edits to existing Check. The job guard rejects unexpected forms before hosted package setup; the captured branch and label are checked as data, never interpolated into shell commands. Temporary outputs remain only within the runner, apart from test logs.

Run only the non-GUI harness-contract checks on any computer without changing packages:

```bash
python3 -B -m unittest discover -v -s scripts/native-desktop-smoke
```
