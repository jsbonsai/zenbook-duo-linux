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
