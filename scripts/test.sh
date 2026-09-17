#!/usr/bin/env bash

# Reclaim age-expired build/test caches before building. No-op when the shared
# helper is not installed, so CI and other machines are unaffected.
command -v disk-clean >/dev/null 2>&1 && disk-clean || true

echo "=== FORMAT ===" &&
cargo fmt --all &&
echo "=== CHECK ===" &&
cargo check &&
echo "=== CLIPPY ===" &&
cargo clippy --tests -- -D warnings &&
echo "=== TEST ===" &&
cargo nextest run &&
echo "=== FORGOTTEN TODOS ===" &&
# Requires ripgrep
! rg -i -s -g '*.rs' 'TODO:|todo:|FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' . &&
echo "=== SUCCESS ===" &&
echo "=== CLEANING FILES ===" &&
./scripts/testcfg_clean.sh &&
echo "=== Done ===."
