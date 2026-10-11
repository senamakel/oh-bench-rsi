#!/usr/bin/env bash
# bootstrap.sh — check out the pinned submodules and prepare the loop.
#
# Initializes vendor/openhuman (recursively, so TinyMemory and the other
# vendored crates are present), vendor/tinyrsi (recursively), and benchmarks/
# (top level only: its own nested vendor/openhuman copy is not used; candidates
# are TinyMemory worktrees of this repository's vendor/openhuman instead).
#
# Then creates the local `rsi/main` branch in the vendored TinyMemory at its
# pinned commit if it does not exist yet (ADR 0004: the incumbent lives on a
# local branch that promotion fast-forwards; nothing is pushed), and validates
# the loop folder with the `tinyrsi` binary.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
git submodule update --init vendor/openhuman vendor/tinyrsi benchmarks
git -C vendor/openhuman submodule update --init --recursive --depth 1
git -C vendor/tinyrsi submodule update --init --recursive --depth 1
tinymemory=vendor/openhuman/vendor/tinymemory
if ! git -C "$tinymemory" rev-parse --verify --quiet refs/heads/rsi/main >/dev/null; then
  git -C "$tinymemory" branch rsi/main HEAD
  echo "created rsi/main in $tinymemory at $(git -C "$tinymemory" rev-parse --short HEAD)"
fi
# `check` only needs the key's name to exist; it never reads its value.
OPENROUTER_API_KEY="${OPENROUTER_API_KEY:-check-only}" \
  cargo run -q --manifest-path vendor/tinyrsi/Cargo.toml -p tinyrsi-cli -- check loops/lme2-recall
