#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$task_root"
python3 tools/prepare-plugin-sdk.py
./tools/build.sh
python3 tools/package-plugin-ui.py
export RUSTC="$(rustup which rustc)"
cargo_bin="$(rustup which cargo)"
case "$(uname -s)" in
 Darwin)
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  for task_arch in aarch64-apple-darwin x86_64-apple-darwin; do
    "$cargo_bin" build --locked --release --manifest-path plugin/engine/Cargo.toml --target "$task_arch"
  done
  mkdir -p target/universal
  lipo -create plugin/engine/target/aarch64-apple-darwin/release/liblxr02_plugin.a plugin/engine/target/x86_64-apple-darwin/release/liblxr02_plugin.a -output target/universal/liblxr02_plugin.a
  cmake -S plugin -B target/plugin-build -DCMAKE_BUILD_TYPE=Release -DIPLUG2_UNIVERSAL=ON -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 -DLXR_RUST_LIBRARY="$task_root/target/universal/liblxr02_plugin.a"
  cmake --build target/plugin-build --parallel 6
  mkdir -p build/plugins/dxm088-macos-universal examples
  for task_suffix in vst3 component clap; do
    ditto "target/plugin-build/out/DXM-088.$task_suffix" "build/plugins/dxm088-macos-universal/DXM-088.$task_suffix"
    codesign --force --sign - "build/plugins/dxm088-macos-universal/DXM-088.$task_suffix"
    codesign --verify --strict "build/plugins/dxm088-macos-universal/DXM-088.$task_suffix"
  done
  "$cargo_bin" run --locked --offline --quiet --release --manifest-path plugin/engine/Cargo.toml --example performance_demo -- "examples/Polyrhythm Scenes.dxm088.json"
  python3 tools/package-plugin-release.py
  python3 tools/build-installer.py
  ;;
 *) printf '%s\n' 'Windows: tools/build-plugins.ps1';exit 1;;
esac
