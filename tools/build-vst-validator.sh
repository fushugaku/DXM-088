#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$task_root"
python3 tools/prepare-plugin-sdk.py
# SDK 3.8.1 probes Xcode even for Makefiles. Command Line Tools can build the validator.
python3 - <<'PY'
from pathlib import Path
p=Path('.deps/iplug2/Dependencies/IPlug/VST3_SDK/cmake/modules/SMTG_Global.cmake')
s=p.read_text();call='\nsmtg_detect_xcode_version()\n';guard='\nif(XCODE)\n    smtg_detect_xcode_version()\nendif()\n'
if call in s:p.write_text(s.replace(call,guard,1))
elif guard not in s:raise SystemExit('Unexpected SDK Xcode detection; review required')
PY
cmake -S .deps/iplug2/Dependencies/IPlug/VST3_SDK -B target/vst-validator -DCMAKE_BUILD_TYPE=Release -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF
cmake --build target/vst-validator --target validator --parallel 6
