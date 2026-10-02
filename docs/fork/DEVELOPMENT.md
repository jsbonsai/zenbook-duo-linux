# Developing this fork

The fork preserves upstream Git history and its GPL license. The initial upstream base is da2d09dfd29bbbd93e820cbc7e4a82cb5f4a0f5e. Local additions were recovered from a CachyOS live-session backup; machine credentials, browser profiles, agent histories and hardware logs are not part of this repository.

From `ui-tauri-react`:

```sh
npm ci
npm run vite:build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
npm run dev
npm run build:local
cargo build --locked --release --manifest-path src-tauri/Cargo.toml --bin zenbook-duo-daemon --bin zenbook-duo-usb-remap-helper
```

Use a working Rust toolchain. On the recovery machine, packaged rustc fails with an LLVM symbol error; the official rustup toolchain works. Run `source ~/.cargo/env` first there. Cargo targets and npm dependencies are disposable build artifacts and excluded from Git. Historical live-session build helpers outside the repository should not be reused as native installation scripts.

Runtime source is under `ui-tauri-react/src-tauri/src/runtime`, hardware access under `hardware`, and privileged IPC requests under `ipc`. The React Power page and Tauri adapter expose the local power extension. Desktop operations belong in the session agent rather than the root daemon.

Initial validation: 138 Rust library tests pass, including recovered-PID and stop-process regression checks; TypeScript/Vite production build passes; optimized daemon and USB remapper builds pass. USB detach/redock, physical media keys and touchscreen mapping need user validation after installing the patched binaries. Existing npm dependency audit reported 23 findings; remediation remains outstanding.

Read [SPRINT.md](SPRINT.md) for feature scope and acceptance checks. Keep upstream `main` as the comparison source and push local development to this fork. No upstream pull request is implied by maintaining this fork.

## v0.4.0 desktop sprint

Desktop & keyboard now provides independent local-image wallpaper selection/previews for eDP-1 and eDP-2, a shared-wallpaper option, configurable F7/F8/F12 actions, Konsole/Alacritty Codex launching, and extra native Plasma OSD feedback. Global shortcuts default to Meta+Shift+D (controls), Meta+Shift+C (Codex), and Meta+Shift+S (swap windows); KWin exposes their bindings in System Settings. F8 defaults to swapping normal application windows between the built-in panels; position swapping and stacked/side-by-side layouts are alternatives. The Status page now includes firmware profile, turbo, temperature, fans, battery and charge ceiling.

The keyboard touchpad on the evaluation UX8406CA reports no native disable-while-typing support. A passive user-session event observer therefore temporarily disables only tap-to-click on the Duo keyboard touchpad after text/editing key presses. It never grabs input, retains typed characters, disables pointer movement or changes physical clicks. The default interval is 750 ms, configurable from 200–2000 ms. The toggle saves automatically; other desktop preferences use Save & apply. Original tap preferences are retained by device name in a recovery file that survives logout/reboot and restored after idle, disable, or service restart. A user tested the preview and reported the typing guard works well. This is tap suppression rather than palm classification; it does not promise to prevent every simultaneous touch/key event.

Desktop settings live in ~/.config/zenbook-duo/desktop.json, independently from the existing Duo layout/power preferences. The session agent owns org.jsbonsai.ZenbookDuo on the user D-Bus and exposes a narrow Action method. The privileged USB/Bluetooth handlers route desktop actions through that user service; only known action names and terminal choices are supported. Plasma scripting maps wallpapers by connector, and the session maintenance loop reapplies selected images after display/activity changes. KWin touch-device assignments are repaired on input-device changes. Wallpaper scripting JSON-escapes file URLs. Wallpaper selections remain empty by default so existing wallpapers are retained.

USB media mode maps F9 to touchpad toggle, F10 to mic mute and F11 to the Plasma emoji picker. Configured “none” passes F7/F8/F12 through in USB mode. Existing native volume/mute/brightness feedback remains desktop-owned to avoid duplicate OSD; keyboard lighting, microphone and layout/window actions request Plasma OSD explicitly. Bluetooth vendor reports 0x9c and 0x86 have actions for screen swap and Codex; real keyboard firmware/Fn-lock behavior still needs physical validation. F7/F12 icon identity is not inferred from generic ASUS hotkey tables. Non-KDE desktop feature parity is outside this sprint.

Build and update an existing Duo installation:

```sh
# From repository root, after setting up a working Rust toolchain:
npm --prefix ui-tauri-react ci
npm --prefix ui-tauri-react run vite:build
cargo test --locked --manifest-path ui-tauri-react/src-tauri/Cargo.toml --lib
cargo build --locked --release --features tauri/custom-protocol --manifest-path ui-tauri-react/src-tauri/Cargo.toml --bins
(cd ui-tauri-react/src-tauri/target/release && sha256sum zenbook-duo-control zenbook-duo-daemon zenbook-duo-lifecycle zenbook-duo-session-agent zenbook-duo-usb-remap-helper > fork-build.sha256)
bash install-fork-build.sh
```

The installer verifies the built payload, checks desktop prerequisites, retains previous binaries, installs brightness authorization with visudo validation, stops the old UI/session/daemon, atomically replaces binaries, restarts services and opens the app. It preserves user settings and requires an existing Duo service installation. Failed install attempts restart services via an exit trap. Cached builds are excluded from Git.

Validation: 144 Rust library tests pass; frontend production and complete custom-protocol release builds pass; installer shell checks pass. The old installer source check was corrected to tolerate Rust formatting/trailing commas in version hooks. Native D-Bus service and shortcuts run in preview; keyboard-lighting OSD call succeeds; window-swap action was exercised twice without script errors. Rendered v0.4.0 navigation was visually inspected. Full post-install USB/Bluetooth media-key/HUD, wallpaper selection/docking, restart and suspend/resume validation remains pending with the user.

KDE API references: https://develop.kde.org/docs/plasma/scripting/api/ and https://develop.kde.org/docs/plasma/kwin/api/ .
