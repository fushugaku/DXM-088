#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")/.." && pwd)"
task_source="$task_root/build/plugins/macos-universal"
for task_pair in 'vst3:VST3' 'component:Components' 'clap:CLAP'; do
 task_ext=${task_pair%:*};task_folder=${task_pair#*:}
 task_dest="$HOME/Library/Audio/Plug-Ins/$task_folder/DXM-088.$task_ext"
 test -d "$task_source/DXM-088.$task_ext"
 if [ -e "$task_dest" ]; then
  task_id=$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$task_dest/Contents/Info.plist")
  case "$task_id" in local.ErikaRust.*.LXR02) ;; *) printf 'Different plug-in already exists: %s\n' "$task_dest";exit 1;;esac
 fi
 mkdir -p "$HOME/Library/Audio/Plug-Ins/$task_folder"
 ditto "$task_source/DXM-088.$task_ext" "$task_dest"
done
printf '%s\n' 'Installed LXR02 VST3 / AU / CLAP into ~/Library/Audio/Plug-Ins/'
