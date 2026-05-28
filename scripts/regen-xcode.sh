#!/usr/bin/env bash
# Regenerate ios/ClearCam.xcodeproj from project.yml.
#
#     ./scripts/regen-xcode.sh
#
# Requires xcodegen (install via `brew install xcodegen`).

set -euo pipefail

if ! command -v xcodegen >/dev/null 2>&1; then
  echo "xcodegen not installed. Run: brew install xcodegen" >&2
  exit 1
fi

cd "$(dirname "$0")/.."
cd ios
xcodegen generate
echo "Generated ClearCam.xcodeproj — open it in Xcode and set your DEVELOPMENT_TEAM."
