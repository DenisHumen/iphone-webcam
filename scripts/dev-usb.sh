#!/usr/bin/env bash
set -euo pipefail
# Starts desktop with the LoopbackConductor pointed at a mock-iphone listener.
#
# NOTE: The env-var → LoopbackConductor wiring on the AppCore side is deferred
# to Phase 6 (start_usb_supervisor currently stubs out the supervisor as noted
# in Task 20). This script is shipped now so that, once wiring lands, no script
# changes are needed.
cd "$(dirname "$0")/.."
( cargo run -p mock-iphone --bin mock-iphone -- --host 127.0.0.1 --cport 17000 --mport 17001 --token devtoken --transport usb ) &
mock_pid=$!
trap "kill $mock_pid 2>/dev/null || true" EXIT
sleep 0.3
CLEARCAM_USB_LOOPBACK=1 \
CLEARCAM_USB_CPORT=17000 \
CLEARCAM_USB_MPORT=17001 \
CLEARCAM_USB_UDID=DEVICE-DEV \
cargo tauri dev --manifest-path desktop/src-tauri/Cargo.toml
