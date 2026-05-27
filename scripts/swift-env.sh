#!/usr/bin/env bash
# Source this file before running `swift build` / `swift test` on macOS
# if your default toolchain is Command Line Tools (mismatched with the system Swift).
#
#     . scripts/swift-env.sh
#     swift build
#
# Or run inline:  DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift build
set -euo pipefail

if [ -d "/Applications/Xcode.app/Contents/Developer" ]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
else
  echo "scripts/swift-env.sh: /Applications/Xcode.app not found — falling back to default toolchain" >&2
fi
