# Task

Fix `tod/scripts/test.sh` so the CI gate can actually fail. The script is one long
`&&` chain, but the forgotten-TODOs check in the middle is an `if` block, which
terminates the chain: everything after it runs unconditionally, so the script's exit
status is always 0 (the trailing `echo`). A gate that cannot fail reports green while
regressions land.

Replace the `if … then exit 1; fi` block with the `! rg … &&` form so the chain stays
intact and any failing step (including a failing `cargo nextest run`) makes the whole
script exit non-zero:

```bash
echo "=== FORGOTTEN TODOS ===" &&
# Requires ripgrep
! rg -i -s -g '*.rs' 'TODO:|todo:|FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' . &&
echo "=== SUCCESS ===" &&
echo "=== CLEANING FILES ===" &&
./scripts/testcfg_clean.sh &&
echo "=== Done ===."
```

Important: `! rg … &&` means the chain is non-zero when ripgrep FINDS a match (exit 0)
but stays zero when it finds none (exit 1). The two genuinely-failing `tod` tests
mentioned in the ticket (`test_process_with_filter`, `test_process_with_project`) are
NOT in scope — they are either fixed separately or tracked separately; do not fix them
here. The current `test.sh` has a bigger problem than those tests: it cannot fail at
all, and that is the bug being fixed.

## Why SMALL

One file (`tod/scripts/test.sh`), follows the existing `! cmd &&` pattern vardy already
uses (the exact fix is spelled out in the ticket). No schema, no new subsystem, no
design decision. The DoD verification (deliberately fail a step, observe non-zero exit)
is local to this file.

## Key files

- `tod/scripts/test.sh` — the only file that should change.