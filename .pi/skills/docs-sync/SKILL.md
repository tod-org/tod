---
name: docs-sync
description: Keep docs/usage.md in sync when adding or modifying CLI commands. Use after implementing a feature that changes the CLI surface area.
---

# Docs Sync

When adding or modifying CLI commands (new subcommands, renamed flags, changed aliases), check `docs/usage.md` for stale example output and update it.

## Checklist

1. Run `tod <changed-command> -h` to get current help output
2. Search `docs/usage.md` for the old command listing (the `Commands:` block in the code fence)
3. Update the example output to match current help text
4. Commit the doc change alongside the code change — same branch, same PR