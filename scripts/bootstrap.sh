#!/usr/bin/env bash
# bootstrap.sh — check out the pinned submodules this repository needs.
#
# Initializes vendor/openhuman (recursively, so TinyMemory and the other
# vendored crates are present), vendor/tinyrsi (recursively), and benchmarks/
# (top level only: its own nested vendor/openhuman copy is not used; candidates
# are measured against this repository's vendor/openhuman instead).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git submodule update --init vendor/openhuman vendor/tinyrsi benchmarks
git -C vendor/openhuman submodule update --init --recursive --depth 1
git -C vendor/tinyrsi submodule update --init --recursive --depth 1
cargo run -q -p ohrsi -- check
