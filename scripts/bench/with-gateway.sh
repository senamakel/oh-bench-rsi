#!/usr/bin/env bash
# with-gateway.sh — run a command behind a fresh spend gateway for one run.
#
# Usage: scripts/bench/with-gateway.sh <run-id> <run-limit-usd> -- <command...>
#
# Starts ohrsi-gateway on 127.0.0.1:8790 with runs/<run-id>/budget.json capped
# at <run-limit-usd> and the campaign-wide runs/ledger-total.json capped at $50,
# exports OHRSI_GATEWAY and OHRSI_RUN_DIR for the command, and stops the
# gateway (releasing its ledger lock) when the command exits.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
run_id="$1"; limit="$2"; shift 2
[ "${1:-}" = "--" ] && shift
run_dir="runs/$run_id"
mkdir -p "$run_dir"
cargo build -q -p ohrsi-gateway
./target/debug/ohrsi-gateway --run-dir "$run_dir" --ledger-total runs/ledger-total.json \
  --run-limit "$limit" --total-limit 50 --listen 127.0.0.1:8790 --capture \
  >"$run_dir/gateway.log" 2>&1 &
gateway=$!
trap 'kill -INT $gateway 2>/dev/null; wait $gateway 2>/dev/null || true' EXIT
for _ in $(seq 1 100); do
  grep -q listening "$run_dir/gateway.log" 2>/dev/null && break
  kill -0 $gateway 2>/dev/null || { cat "$run_dir/gateway.log" >&2; exit 1; }
  sleep 0.2
done
export OHRSI_GATEWAY=http://127.0.0.1:8790 OHRSI_RUN_DIR="$run_dir"
"$@"
