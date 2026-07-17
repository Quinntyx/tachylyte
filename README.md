# Tachylyte

Tachylyte is a native Rust knowledge-workspace shell. The bootstrap desktop
frame is rendered with the crates.io release `gpui = "=0.2.2"`; it contains a
title/tab strip, ribbon, collapsible sidebar state, editor placeholder, status
bar, and a core-feature settings panel.

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
cargo run -p tachylyte-app
cargo fmt --check
cargo check -p tachylyte-app --all-targets
cargo test -p tachylyte-app
cargo clippy -p tachylyte-app --all-targets -- -D warnings
```

The workspace uses `crates/*` membership so future crates are discovered
automatically without coupling this shell to them. `Cargo.lock` is committed
to make the GPUI and transitive dependency graph reproducible.
