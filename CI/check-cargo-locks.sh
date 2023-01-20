#!/usr/bin/env bash

# Runs the flatpak Cargo.lock -> flatpak crate dependency generator

set -o errexit
set -o pipefail

python3 CI/flatpak/scripts/flatpak-cargo-generator.py ./plugins/obs-webrtc/webrtc/Cargo.lock -o CI/flatpak/webrtc-cargo-sources.json
