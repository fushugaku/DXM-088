#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$task_root"
if command -v rustup >/dev/null 2>&1; then
    export RUSTC="$(rustup which rustc)"
    export RUSTDOC="$(rustup which rustdoc)"
    cargo_bin="$(rustup which cargo)"
else
    cargo_bin=cargo
fi
"$cargo_bin" build --locked --release --target wasm32-unknown-unknown --lib
cp target/wasm32-unknown-unknown/release/lxr02.wasm web/lxr02.wasm
"$cargo_bin" build --locked --release --bin dxm088
