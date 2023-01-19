#!/usr/bin/env bash

set -o errexit
set -o pipefail

python3 CI/flatpak/scripts/flatpak-cargo-generator.py  ./plugins/obs-webrtc/webrtc/Cargo.lock -d -o CI/flatpak/obs-cargo-sources.json