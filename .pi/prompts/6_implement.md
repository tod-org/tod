---
description: Execute the plan phase by phase with verification checkpoints
argument-hint: ".pi/qrspi/<id>/"
---

# Implement — Execute the Plan

Implement the plan one phase at a time, verifying each phase before proceeding. Update the plan's checkboxes as you go — they are your progress tracker and context-recovery mechanism.

## Input

Read `$ARGUMENTS/plan.md`. That is your primary working document.

## Process

1. **Read `plan.md` fully.** Check for existing checkmarks (`- [x]`) — if some phases are already complete, pick up from the first unchecked item.

2. **Read all files referenced in the current phase** before making changes. Understand the code you're modifying.

3. **Implement one phase at a time:**
   - Make the changes described in the plan
   - Follow the plan's intent, but adapt if the codebase has diverged from what the plan expected
   - If you hit a mismatch, stop and present it:
     ```
     Issue in Phase [N]:
     Expected: [what the plan says]
     Found: [actual situation]
     Impact: [what this means for the plan]

     How should I proceed?
     ```

4. **After completing a phase, run verification:**
   - Execute the automated verification commands from the plan
   - Fix any failures before proceeding
   - `/scripts/test.sh` must pass
   - Check off automated items in `plan.md` using Edit: `- [ ]` becomes `- [x]`

5. **Create a branch if on main** If on `main` branch, create a new subticket underneath the ticket given, a new branch for that ticket, and open up a PR.
 
6. **Commit the phase** after automated verification passes. Each phase should be a separate commit so it can be independently reverted if later phases break something. Use a descriptive message like `"Phase N: [phase name from plan]"`.

7. **Pause for manual verification** (unless told to continue through multiple phases):
   ```
   Phase [N] complete — ready for manual verification.

   Automated checks passed:
   - [x] [list what passed]

   Please verify manually:
   - [ ] [manual items from the plan]

   Let me know when done, and I'll proceed to Phase [N+1].
   ```

8. **Repeat** for each phase until the plan is complete.

## Resuming After Context Reset

If you're starting fresh in a new context window:
- Read `plan.md` — checked boxes show what's done
- Trust completed work unless something seems off
- Pick up from the first unchecked item

## Output

- Code changes implemented according to the plan
- `plan.md` updated with checked verification items

## Rules

- One phase at a time. Do not skip ahead.
- Read before you write. Understand existing code before changing it.
- Update checkboxes as you go — they are the source of truth for progress.
- Do not check off manual verification items until the user confirms.
- If the plan has errors, stop and ask. Do not silently deviate.
- Only make changes described in the plan. Do not refactor, clean up, or "improve" code you encounter along the way — even if it's messy. If you see something worth fixing, note it for the user after the phase is done.
- Use sub-agents sparingly — only for targeted debugging or exploring unfamiliar code.
- Commit after each phase passes automated verification — one commit per phase.

## When to Go Back

If a phase reveals the plan is fundamentally wrong — not a small mismatch but a structural issue like a missing dependency, wrong API, or incorrect assumption about the codebase — tell the user. For small mismatches, adapt and continue. For fundamental issues, suggest re-running `/5_plan` or even `/3_design` with the new information rather than building on a broken foundation.
