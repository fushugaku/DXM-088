#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$task_root"
task_app="$task_root/target/LXR02PluginTest.app"
mkdir -p "$task_app/Contents/MacOS"
clang++ -std=c++17 -fobjc-arc -O2 -framework Cocoa -I.deps/iplug2/Dependencies/IPlug/CLAP_SDK/include plugin/tests/editor_host.mm -o "$task_app/Contents/MacOS/lxr-editor-host"
python3 - "$task_app" "$task_root" <<'PY'
import plistlib,sys
from pathlib import Path
app=Path(sys.argv[1]);root=Path(sys.argv[2])
(app/'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable':'lxr-editor-host','CFBundleIdentifier':'local.ErikaRust.LXR02Test','CFBundleName':'DXM-088 Plugin Test','CFBundleVersion':'0.9.0','LXR02PluginBinary':str(root/'target/plugin-build/out/DXM-088.clap/Contents/MacOS/DXM-088'),'LXR02QAFolder':str(root/'artifacts'),'NSHighResolutionCapable':True}))
PY
codesign --force --sign - "$task_app"
