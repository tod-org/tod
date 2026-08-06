---
description: Decompose a task into neutral research questions
argument-hint: "<ticket file, issue URL, or task description>"
---

# Question — Decompose the Task

Transform a task description into 3-7 specific, neutral research questions. These questions drive the next phase (Research) which runs in a **separate context with no knowledge of what is being built**.

## Input

The user provides a task description, ticket file path, or issue reference.

## Process

1. **Read any provided files fully** before doing anything else.

2. **Light codebase exploration**: Spawn a **codebase-locator** agent to find which areas of the codebase relate to the task. You need to know what exists to write good questions.

3. **Decompose into 3-7 research questions**:
   - Each question should cause a researcher to explore a different relevant area of the codebase
   - Questions must be **neutral** — they ask what exists and how it works, never how to build something
   - Prefer "trace the flow" questions that reveal architecture over yes/no questions

   Good: "How does the middleware chain handle request authentication, and where are auth policies defined?"
   Bad: "What's the best way to add a new authenticated endpoint?"

   Good: "What patterns exist for database migrations, and how are they tested?"
   Bad: "How should we add a new migration for the users table?"

4. **Determine the artifact directory**:
   - With ticket number: `.pi/qrspi/PROJ-1234-brief-description/` (use the project's ticket prefix)
   - Without ticket: `.pi/qrspi/YYYY-MM-DD-brief-description/`

5. **Create the artifact directory** if it doesn't exist (e.g., `mkdir -p .pi/qrspi/<id>/`).

6. **Write `task.md`** — a clean 2-3 sentence description of what's being built and why. This file persists the task context for later phases so the user doesn't have to re-explain it.

7. **Write `questions.md`** to the artifact directory:

   ```markdown
   # Research Questions

   ## Context
   [2-3 sentences describing which areas of the codebase to focus on.
   Do NOT mention what is being built or why.]

   ## Questions
   1. [Neutral, fact-seeking question]
   2. [Neutral, fact-seeking question]
   ...
   ```

8. **Present questions to the user** and wait for approval or edits before finalizing.

9. Create a new subticket under the main ticket, create a branch for it and open a PR with "design" in the title.

## Output

- Directory created: `.pi/qrspi/<id>/`
- Files written: `.pi/qrspi/<id>/task.md` and `.pi/qrspi/<id>/questions.md`
- Tell the user: "Next: run `/2_research .pi/qrspi/<id>/`"

## Rules

- `questions.md` must NOT contain the task description, goals, or desired behavior
- `task.md` is a brief, honest description of the goal — it will be read by later phases but NOT by Research
- The researcher who reads these questions should have no idea what feature is being built
- Each question should target a different area or concern
- If the task is too simple for 3 questions, tell the user — QRSPI is for complex tasks
