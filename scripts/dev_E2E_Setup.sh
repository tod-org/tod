#!/usr/bin/env bash
# seed_static_fixture.sh - (Re)creates the TOD_DEV_CI_STATIC fixture project
# used by the read-only E2E tests in .github/workflows/e2e_todoist.yml.
#
# This is the executable counterpart to TOD_DEV_CI_STATIC.yml - if that file
# ever changes, update this script to match, and vice versa.
#
# Usage:
#   ./seed_static_fixture.sh <TODOIST_API_TOKEN>
#
# Refuses to run if a project with this name already exists, so it can't
# silently create a duplicate or overwrite live fixture data.
#
# LIMITATION: `tod task create` has no flag to place a task directly into a
# section (only --no-section, which skips the prompt). The two
# section-scoped tasks are created without a section, then this script
# prints instructions for the one manual step: drag them into
# "Static Section" in the Todoist app. Everything else is fully scripted.

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <TODOIST_API_TOKEN>" >&2
  exit 1
fi

TODOIST_API_TOKEN="$1"
TOD_BIN="${TOD_BIN:-tod}"
TOD_STATIC_PROJECT="TOD_DEV_CI_STATIC"
TMP_CONFIG=$(mktemp)
trap 'rm -f "$TMP_CONFIG"' EXIT

echo "=== Authenticating ==="
"$TOD_BIN" --config "$TMP_CONFIG" auth token "$TODOIST_API_TOKEN"
"$TOD_BIN" --config "$TMP_CONFIG" project import --auto

if "$TOD_BIN" --config "$TMP_CONFIG" project list | grep -q "$TOD_STATIC_PROJECT"; then
  echo "ERROR: $TOD_STATIC_PROJECT already exists. Refusing to run to avoid" >&2
  echo "creating a duplicate or clobbering live fixture data. Delete or" >&2
  echo "rename the existing project first if you really want to reseed." >&2
  exit 1
fi

echo "=== Creating project ==="
"$TOD_BIN" --config "$TMP_CONFIG" project create -n "$TOD_STATIC_PROJECT"

echo "=== Creating section ==="
"$TOD_BIN" --config "$TMP_CONFIG" section create -n "Static Section" -p "$TOD_STATIC_PROJECT"

echo "=== Creating root-level tasks ==="
"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] Overdue High Priority" \
  --due 2020-01-01 --priority 4 --no-section

"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] Overdue Medium Priority" \
  --due 2020-06-15 --priority 3 --no-section

"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] Future Low Priority Labeled" \
  --due 2099-12-31 --priority 2 --label e2estatic --no-section

"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] No Date No Label" \
  --priority 1 --no-section

echo "=== Creating section-bound tasks (without section - see note below) ==="
"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] Section Task No Date" \
  --priority 1 --no-section

"$TOD_BIN" --config "$TMP_CONFIG" task create \
  --project "$TOD_STATIC_PROJECT" \
  --content "[E2E-STATIC] Section Task Future High Priority Labeled" \
  --due 2099-06-15 --priority 4 --label e2estatic --no-section

echo
echo "=== Done - one manual step remains ==="
echo "tod cannot assign a task to a section on creation (no --section flag)."
echo "Open $TOD_STATIC_PROJECT in the Todoist app and drag these two tasks"
echo "into 'Static Section':"
echo "  - [E2E-STATIC] Section Task No Date"
echo "  - [E2E-STATIC] Section Task Future High Priority Labeled"
echo
echo "Once done, verify with:"
echo "  tod --config <config> list view --filter \"#$TOD_STATIC_PROJECT & /Static Section\""
echo "which should return exactly those two tasks."
