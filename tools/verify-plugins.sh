#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$task_root"
export RUSTC="$(rustup which rustc)"
export RUSTDOC="$(rustup which rustdoc)"
cargo_bin="$(rustup which cargo)"
"$cargo_bin" test --locked > artifacts/core-plugin-regression.log 2>&1
"$cargo_bin" test --locked --manifest-path plugin/engine/Cargo.toml > artifacts/plugin-rust-tests.log 2>&1
task_bundles="$task_root/build/plugins/dxm088-macos-universal"
case "$(uname -m)" in arm64) task_target=aarch64-apple-darwin;; x86_64) task_target=x86_64-apple-darwin;; esac
task_library="plugin/engine/target/$task_target/release/liblxr02_plugin.a"
for task_ext in vst3 component clap; do
 codesign --verify --strict "$task_bundles/DXM-088.$task_ext"
 lipo -archs "$task_bundles/DXM-088.$task_ext/Contents/MacOS/DXM-088"
done
clang++ -std=c++17 -O2 -I .deps/iplug2/Dependencies/IPlug/CLAP_SDK/include tools/plugin-host-tests.mm "$task_library" -framework AudioToolbox -framework AudioUnit -framework CoreFoundation -framework Foundation -framework Security -framework SystemConfiguration -liconv -o target/plugin-host-tests
./target/plugin-host-tests "$task_bundles/DXM-088.component" "$task_bundles/DXM-088.clap/Contents/MacOS/DXM-088" > artifacts/plugin-host-tests.log 2>&1
if [ ! -x target/vst-validator/bin/Release/validator ];then ./tools/build-vst-validator.sh > artifacts/vst3-validator-build.log 2>&1;fi
clang++ -std=c++17 -O2 -DNDEBUG -I .deps/iplug2/Dependencies/IPlug/VST3_SDK tools/plugin-vst3-host-tests.mm .deps/iplug2/Dependencies/IPlug/VST3_SDK/public.sdk/source/common/memorystream.cpp target/vst-validator/lib/Release/libsdk_hosting.a target/vst-validator/lib/Release/libsdk_common.a target/vst-validator/lib/Release/libbase.a target/vst-validator/lib/Release/libpluginterfaces.a "$task_library" -framework Cocoa -framework Security -framework SystemConfiguration -liconv -o target/plugin-vst3-host-tests
./target/plugin-vst3-host-tests "$task_bundles/DXM-088.vst3" > artifacts/plugin-vst3-host-tests.log 2>&1
target/vst-validator/bin/Release/validator "$task_bundles/DXM-088.vst3" > artifacts/vst3-validation.log 2>&1
cat artifacts/plugin-host-tests.log artifacts/plugin-vst3-host-tests.log
tail -n 3 artifacts/vst3-validation.log
