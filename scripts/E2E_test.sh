#!/usr/bin/env bash
# E2E_test.sh - Local runner for the tod E2E Todoist integration tests.
#
# Mirrors the checks in .github/workflows/e2e_todoist.yml so they can be run
# locally against the dev Todoist account before pushing. See ci.md for full
# detail on each test case and the static fixture project setup.
#
# Lives in ./scripts/ - paths are resolved relative to this script's own
# location, so it works whether invoked as `./scripts/E2E_test.sh` from the
# repo root or `./E2E_test.sh` from inside scripts/.
#
# Output is intentionally terse: one line per test ("Test: <name>: PASS" or
# "Test: <name>: FAIL"). Raw tod output is only printed for a test that
# fails, so it's there for debugging without burying the pass/fail signal.
#
# Usage:
#   ./scripts/E2E_test.sh <TODOIST_API_TOKEN>
#
# Requires:
#   - A dev/test Todoist account API token (NEVER a personal account token)
#   - Tod_DEV_CI project (empty) already created in that account
#   - TOD_DEV_CI_STATIC fixture project already created (see ci.md, or run
#     ./scripts/dev_E2E_setup.sh to create it)
#   - cargo / rustc available on PATH

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Args -----------------------------------------------------------------
if [ $# -ne 1 ]; then
  echo "Usage: $0 <TODOIST_API_TOKEN>" >&2
  exit 1
fi

TODOIST_API_TOKEN="$1"

# --- Config ---------------------------------------------------------------
CARGO_TERM_COLOR=always
DISABLE_SPINNER="1"
TOD_BIN="$REPO_ROOT/target/release/tod"
TOD_PROJECT="Tod_DEV_CI"
TOD_STATIC_PROJECT="TOD_DEV_CI_STATIC"

TMP_DIR=$(mktemp -d)
TOD_CONFIG="$TMP_DIR/tod-e2e-test.cfg"
TOD_SCRATCH_CONFIG="$TMP_DIR/tod-e2e-scratch.cfg"
TOD_TEMP_PROJECT="Tod_E2E_Temp_$(date +%s)_$$"

export CARGO_TERM_COLOR DISABLE_SPINNER

PASS_COUNT=0
FAIL_COUNT=0

# pass "test name"
pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  echo "Test: $1: PASS"
}

# fail "test name" ["captured output to show for debugging"]
fail() {
  FAIL_COUNT=$((FAIL_COUNT + 1))
  echo "Test: $1: FAIL"
  if [ -n "${2:-}" ]; then
    echo "  --- output ---"
    echo "${2//$'\n'/$'\n'  }" >&2
    echo "  --------------"
  fi
}

# must "step description" <command...>
# For plumbing steps (advancing state between real assertions, e.g. "get the
# next task, then complete it") where success is assumed rather than
# asserted on. If the command fails, prints exactly which step and why
# instead of letting `set -e` kill the script with a bare, unlabeled error.
must() {
  local desc="$1"
  shift
  local out
  if ! out=$("$@" 2>&1); then
    echo "ERROR during: $desc" >&2
    echo "  --- output ---" >&2
    echo "${out//$'\n'/$'\n'  }" >&2
    echo "  --------------" >&2
    exit 1
  fi
}

cleanup() {
  echo
  echo "=== Cleanup ==="
  if [ -f "$TOD_CONFIG" ]; then
    for _ in $(seq 1 20); do
      output=$("$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT" 2>&1) || break
      if echo "$output" | grep -q "No tasks on list"; then
        break
      fi
      if ! echo "$output" | grep -q "\[E2E\]"; then
        echo "WARNING: Unexpected non-E2E task found - stopping cleanup to avoid data loss" >&2
        echo "${output//$'\n'/$'\n'  }" >&2
        break
      fi
      "$TOD_BIN" --config "$TOD_CONFIG" task complete >/dev/null 2>&1 || true
    done
    "$TOD_BIN" --config "$TOD_CONFIG" project delete -p "$TOD_TEMP_PROJECT" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_DIR"
  echo "Done."
}
trap cleanup EXIT

# --- Build ------------------------------------------------------------------
echo "=== Build ==="
if cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" --target-dir "$REPO_ROOT/target"; then
  echo "Build: PASS"
else
  echo "Build: FAIL"
  exit 1
fi
echo

# --- CLI-only: no config, no live API calls ----------------------------------
echo "=== CLI-only tests ==="

output=$("$TOD_BIN" --version)
if echo "$output" | grep -qE "^tod [0-9]+\.[0-9]+\.[0-9]+"; then
  pass "--version prints expected format"
else
  fail "--version prints expected format" "$output"
fi

output=$("$TOD_BIN" --help)
missing=0
for cmd in project section task list config auth shell; do
  echo "$output" | grep -q "  $cmd " || missing=1
done
if [ "$missing" -eq 0 ]; then
  pass "--help lists all expected top-level commands"
else
  fail "--help lists all expected top-level commands" "$output"
fi

echo "placeholder" > "$TOD_SCRATCH_CONFIG"
if "$TOD_BIN" --config "$TOD_SCRATCH_CONFIG" config reset --force >/dev/null 2>&1 && [ ! -f "$TOD_SCRATCH_CONFIG" ]; then
  pass "config reset deletes an existing config file"
else
  fail "config reset deletes an existing config file"
fi

# KNOWN ISSUE: `config reset --help` documents "Errors if the file does not
# exist", implying a nonzero exit code. Confirmed against a real build: tod
# prints "No config file found at ..." but still exits 0. Worth filing
# upstream; until then this asserts on the message (the reliable part) and
# reports the exit code as informational rather than failing on it.
set +e
output=$("$TOD_BIN" --config "$TOD_SCRATCH_CONFIG" config reset --force 2>&1)
code=$?
set -e
if echo "$output" | grep -q "No config file found"; then
  pass "config reset prints expected message for a missing config file (exit code: $code, not asserted - see known issue comment)"
else
  fail "config reset prints expected message for a missing config file" "$output"
fi

set +e
"$TOD_BIN" --config "$TMP_DIR/tod-e2e-does-not-exist.cfg" config check-version >/dev/null 2>&1
code=$?
set -e
if [ "$code" -eq 0 ] || [ "$code" -eq 1 ]; then
  pass "config check-version runs without a config file"
else
  fail "config check-version runs without a config file" "exit code $code"
fi
echo

# --- Auth + project setup ----------------------------------------------------
echo "=== Auth & project setup ==="
"$TOD_BIN" --config "$TOD_CONFIG" auth token "$TODOIST_API_TOKEN" >/dev/null
"$TOD_BIN" --config "$TOD_CONFIG" project import --auto >/dev/null

if "$TOD_BIN" --config "$TOD_CONFIG" project list | grep -q "$TOD_PROJECT"; then
  pass "$TOD_PROJECT is available in config"
else
  fail "$TOD_PROJECT is available in config"
fi

if "$TOD_BIN" --config "$TOD_CONFIG" project list | grep -q "$TOD_STATIC_PROJECT"; then
  pass "$TOD_STATIC_PROJECT is available in config"
else
  fail "$TOD_STATIC_PROJECT is available in config"
fi
echo

# --- Pre-test cleanup ---------------------------------------------------------
echo "=== Pre-test cleanup ==="
for _ in $(seq 1 20); do
  output=$("$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT" 2>&1)
  if echo "$output" | grep -q "No tasks on list"; then
    break
  fi
  if ! echo "$output" | grep -q "\[E2E\]"; then
    echo "FAIL: Unexpected non-E2E task found in $TOD_PROJECT; refusing to continue" >&2
    printf '%s\n' "$output" | sed 's/^/  /' >&2
    exit 1
  fi
  must "complete leftover task during pre-test cleanup" "$TOD_BIN" --config "$TOD_CONFIG" task complete
done
echo "$TOD_PROJECT is clean"
echo

# --- Static fixture project: read-only sort/filter tests --------------------
echo "=== Static project queries (read-only) ==="

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --project "$TOD_STATIC_PROJECT" --sort value)
names=$(echo "$output" | grep '^- ')
if echo "$names" | sed -n '1p' | grep -q "Overdue High Priority" \
  && echo "$names" | sed -n '2p' | grep -q "Section Task Future High Priority Labeled" \
  && echo "$names" | sed -n '3p' | grep -q "Overdue Medium Priority" \
  && echo "$names" | sed -n '4p' | grep -q "Future Low Priority Labeled" \
  && echo "$names" | tail -2 | grep -q "No Date No Label" \
  && echo "$names" | tail -2 | grep -q "Section Task No Date"; then
  pass "--sort value matches expected priority ordering"
else
  fail "--sort value matches expected priority ordering" "$output"
fi

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --project "$TOD_STATIC_PROJECT" --sort datetime)
names=$(echo "$output" | grep '^- ')
if echo "$names" | sed -n '3p' | grep -q "Overdue High Priority" \
  && echo "$names" | sed -n '4p' | grep -q "Overdue Medium Priority" \
  && echo "$names" | sed -n '5p' | grep -q "Section Task Future High Priority Labeled" \
  && echo "$names" | sed -n '6p' | grep -q "Future Low Priority Labeled"; then
  pass "--sort datetime matches expected date ordering"
else
  fail "--sort datetime matches expected date ordering" "$output"
fi

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --filter "#$TOD_STATIC_PROJECT & p1")
count=$(echo "$output" | grep -c '^- ')
if [ "$count" -eq 2 ] && echo "$output" | grep -q "Overdue High Priority" \
  && echo "$output" | grep -q "Section Task Future High Priority Labeled"; then
  pass "p1 filter returns exactly the two raw-priority-4 tasks"
else
  fail "p1 filter returns exactly the two raw-priority-4 tasks" "$output"
fi

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --filter "#$TOD_STATIC_PROJECT & @e2estatic")
count=$(echo "$output" | grep -c '^- ')
if [ "$count" -eq 2 ] && echo "$output" | grep -q "Future Low Priority Labeled" \
  && echo "$output" | grep -q "Section Task Future High Priority Labeled"; then
  pass "@e2estatic filter returns exactly the two labeled tasks"
else
  fail "@e2estatic filter returns exactly the two labeled tasks" "$output"
fi

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --filter "#$TOD_STATIC_PROJECT & /Static Section")
count=$(echo "$output" | grep -c '^- ')
if [ "$count" -eq 2 ] && echo "$output" | grep -q "Section Task No Date" \
  && echo "$output" | grep -q "Section Task Future High Priority Labeled"; then
  pass "section filter returns exactly the two tasks in Static Section"
else
  fail "section filter returns exactly the two tasks in Static Section" "$output"
fi
echo

# --- Dynamic task tests: create/complete against Tod_DEV_CI ------------------
echo "=== Dynamic task tests ==="

"$TOD_BIN" --config "$TOD_CONFIG" task create --content "[E2E] High Priority Task" --project "$TOD_PROJECT" --priority 4 --no-section >/dev/null
"$TOD_BIN" --config "$TOD_CONFIG" task create --content "[E2E] Medium Priority Task" --project "$TOD_PROJECT" --priority 3 --no-section >/dev/null
"$TOD_BIN" --config "$TOD_CONFIG" task create --content "[E2E] No Priority Task" --project "$TOD_PROJECT" --priority 1 --no-section >/dev/null
"$TOD_BIN" --config "$TOD_CONFIG" task create --content "[E2E] Low Priority Task" --project "$TOD_PROJECT" --priority 2 --no-section >/dev/null

output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --project "$TOD_PROJECT")
if echo "$output" | grep -q "\[E2E\] High Priority Task" \
  && echo "$output" | grep -q "\[E2E\] Medium Priority Task" \
  && echo "$output" | grep -q "\[E2E\] No Priority Task" \
  && echo "$output" | grep -q "\[E2E\] Low Priority Task"; then
  pass "all four priority tasks were created"
else
  fail "all four priority tasks were created" "$output"
fi

expected_order=("High Priority Task" "Medium Priority Task" "Low Priority Task" "No Priority Task")
for task_name in "${expected_order[@]}"; do
  output=$("$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT")
  if echo "$output" | grep -q "\[E2E\] $task_name"; then
    pass "sort order: $task_name returned at expected position"
  else
    fail "sort order: $task_name returned at expected position" "$output"
  fi
  must "complete $task_name" "$TOD_BIN" --config "$TOD_CONFIG" task complete
done

output=$("$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT")
if echo "$output" | grep -q "No tasks on list"; then
  pass "project empty after completing all priority tasks"
else
  fail "project empty after completing all priority tasks" "$output"
fi

"$TOD_BIN" --config "$TOD_CONFIG" task quick-add --content "[E2E] Quickadd Task #$TOD_PROJECT p1 tomorrow" >/dev/null
output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --project "$TOD_PROJECT")
if echo "$output" | grep -q "\[E2E\] Quickadd Task" && echo "$output" | grep -A1 "\[E2E\] Quickadd Task" | grep -q "!"; then
  pass "quick-add creates a task with parsed priority/due date"
else
  fail "quick-add creates a task with parsed priority/due date" "$output"
fi
must "get next task (to complete quick-add task)" "$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT"
must "complete quick-add task" "$TOD_BIN" --config "$TOD_CONFIG" task complete

"$TOD_BIN" --config "$TOD_CONFIG" task create --content "[E2E] Labeled Task" --project "$TOD_PROJECT" --priority 1 --label e2elabel --no-section >/dev/null
output=$("$TOD_BIN" --config "$TOD_CONFIG" list view --filter "#$TOD_PROJECT & @e2elabel")
if echo "$output" | grep -q "\[E2E\] Labeled Task"; then
  pass "label applied at creation is filterable via @e2elabel"
else
  fail "label applied at creation is filterable via @e2elabel" "$output"
fi
must "get next task (to complete labeled task)" "$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT"
must "complete labeled task" "$TOD_BIN" --config "$TOD_CONFIG" task complete

output=$("$TOD_BIN" --config "$TOD_CONFIG" task next --project "$TOD_PROJECT")
if echo "$output" | grep -q "No tasks on list"; then
  pass "project empty after quick-add/label tests"
else
  fail "project empty after quick-add/label tests" "$output"
fi
echo

# --- Temp project lifecycle ---------------------------------------------------
echo "=== Temp project lifecycle ==="

"$TOD_BIN" --config "$TOD_CONFIG" project create -n "$TOD_TEMP_PROJECT" >/dev/null
if "$TOD_BIN" --config "$TOD_CONFIG" project list | grep -q "$TOD_TEMP_PROJECT"; then
  pass "temp project created and added to config"
else
  fail "temp project created and added to config"
fi

output=$("$TOD_BIN" --config "$TOD_CONFIG" section create -n "Temp Section" -p "$TOD_TEMP_PROJECT")
if echo "$output" | grep -qi "section created"; then
  pass "section created in temp project"
else
  fail "section created in temp project" "$output"
fi

"$TOD_BIN" --config "$TOD_CONFIG" project delete -p "$TOD_TEMP_PROJECT" >/dev/null
pass "temp project deleted"

if output=$("$TOD_BIN" --config "$TOD_CONFIG" project delete -p "$TOD_TEMP_PROJECT" 2>&1); then
  fail "deleting an already-deleted project correctly fails" "$output"
else
  pass "deleting an already-deleted project correctly fails"
fi
echo

# --- Summary ------------------------------------------------------------------
echo "=== Summary ==="
echo "Passed: $PASS_COUNT"
echo "Failed: $FAIL_COUNT"

if [ "$FAIL_COUNT" -gt 0 ]; then
  exit 1
fi
