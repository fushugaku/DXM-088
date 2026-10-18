#!/usr/bin/env bash
set -euo pipefail
task_root="$(cd "$(dirname "$0")" && pwd)"
cd "$task_root"
bash tools/build.sh
exec target/release/dxm088 serve "${1:-8732}"
