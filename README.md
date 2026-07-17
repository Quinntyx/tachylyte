# Tachylyte

Tachylyte is a native Rust, local-first knowledge workspace rendered with the
crates.io release `gpui = "=0.2.2"`. It can open an existing vault, scan and
edit Markdown files, save atomically, search and follow knowledge metadata, and
toggle built-in feature surfaces. The workspace also contains reusable GPUI
editor, navigation, Canvas, and Bases components plus render-neutral domain
crates for compatibility behavior.

This is an active compatibility implementation, not a completed 1:1 Obsidian
replacement. See [`PARITY.md`](PARITY.md) for the evidence-backed distinction
between implemented, model-only, and missing behavior.

## Linux prerequisites

Install Rust through [rustup](https://rustup.rs/) and a C toolchain. On Debian
or Ubuntu, install the GPUI windowing and font dependencies before building:

```sh
sudo apt-get install build-essential pkg-config libfontconfig1-dev \
  libx11-xcb-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libwayland-dev
```

The application needs a graphical session (`DISPLAY` or Wayland) at runtime.

## Build, run, and verify

```sh
cargo run -p tachylyte-app -- /path/to/existing/vault
cargo fmt --check
cargo doc --no-deps -p gpui
cargo doc --workspace --no-deps
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace uses `crates/*` membership so future crates are discovered
automatically without coupling this shell to them. `Cargo.lock` is committed
to make the GPUI and transitive dependency graph reproducible.
