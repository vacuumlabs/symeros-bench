#!/usr/bin/env bash
# One process per (variant, scheme, M) row, real proving, peak RSS via /usr/bin/time -v.
# Run from the repository root after building (see README).
# Usage: scripts/run_matrix.sh <patched|unpatched> "<scheme:M ...>"
#   e.g. scripts/run_matrix.sh patched "noop:1 schnorr-secp256k1:5"
set -u
export PATH=$HOME/.risc0/bin:$HOME/.cargo/bin:$PATH
export RISC0_DEV_MODE=0
root=$(cd "$(dirname "$0")/.." && pwd)
variant=$1; shift
case $variant in
  patched) dir=$root ;;
  unpatched) dir=$root/variants/ed25519-unpatched ;;
  *) echo "variant must be patched or unpatched" >&2; exit 2 ;;
esac
out=$root/results
mkdir -p "$out"
for row in $1; do
  s=${row%%:*}; m=${row##*:}
  log=$out/log-$variant-$s-m$m.txt
  echo "=== $(date -Is) $variant $s M=$m" | tee -a "$out/progress.txt"
  /usr/bin/time -v "$dir/target/release/bench" --scheme "$s" --n "$m" --prover cpu \
      --csv "$out/raw-$variant.csv" > "$log" 2>&1
  rc=$?
  rss=$(grep "Maximum resident" "$log" | awk '{print $NF}')
  echo "$variant,$s,$m,$rc,$rss" >> "$out/rss.csv"
  grep "M=$m" "$log" | tail -1 | tee -a "$out/progress.txt"
  echo "rc=$rc peak_rss_kb=$rss" | tee -a "$out/progress.txt"
done
