---
description: Structure outline — vertical slices with test checkpoints
argument-hint: ".pi/qrspi/<id>/"
---

# Structure — How Do We Get There?

Create a ~2-page structure outline that breaks the design into **vertical slices** — each independently testable. Show the signatures, types, and phase boundaries — not the full implementation.

## Input

Read `$ARGUMENTS/design.md` and `$ARGUMENTS/research.md`.

## Process

1. **Read both artifacts fully.**

2. **Break the work into vertical slices.** Each slice delivers end-to-end functionality:
   - Crosses all necessary layers (database, service, API, UI) for that slice
   - Can be tested independently after implementation
   - Has a clear verification checkpoint

   **Vertical** (correct):
   > Phase 1: Add the "reticulate" endpoint — migration, store method, API handler, basic UI button. Test: endpoint returns 200, button triggers call.

   **Horizontal** (wrong):
   > Phase 1: All database migrations. Phase 2: All service methods. Phase 3: All API endpoints. Phase 4: All UI changes.

3. **Define the phase order.** Earlier phases should establish foundations that later phases build on. If Phase 3 fails, Phases 1-2 should still be independently valuable.

4. **For each phase, list**:
   - What it accomplishes (1-2 sentences)
   - Files affected
   - Key type signatures or interface changes
   - How to verify it works (automated command + what to check manually)

5. **Write `structure.md`** to the artifact directory:

   ```markdown
   # Structure Outline

   ## Approach
   [1-2 sentences: the implementation strategy from design.md, condensed]

   ## Phase 1: [Name]
   [What this phase delivers end-to-end]

   **Files**: `path/to/file.ext`, `path/to/other.ext`
   **Key changes**:
   - `functionName(param: Type): ReturnType` — new/modified
   - `NewType { field: Type }` — new type

   **Verify**: [project test command] passes; [manual check description]

   ---

   ## Phase 2: [Name]
   ...

   ## Testing Checkpoints
   [Summary of what should be true after each phase, useful for resuming if context resets]
   ```

6. **Present the outline to the user** and wait for feedback. Common adjustments:
   - Reordering phases
   - Splitting a phase that's too large
   - Adding a testing phase between sensitive phases
   - Requesting more detail on a specific phase

## Output

- File written: `.pi/qrspi/<id>/structure.md`
- Tell the user: "Next: run `/5_plan .pi/qrspi/<id>/`"

## Rules

- ~2 pages max. If it's longer, you're writing the plan, not the outline.
- Vertical slices, not horizontal layers. Every phase must cross all relevant layers.
- Signatures and types, not full implementation. Show WHAT changes, not HOW.
- Each phase must have a verification checkpoint.
- If the design calls for something that can't be sliced vertically, note it explicitly.

## When to Go Back

If you discover the design missed a critical constraint or made a decision based on incorrect assumptions about the codebase, tell the user and suggest re-running `/3_design` rather than working around a flawed design.
