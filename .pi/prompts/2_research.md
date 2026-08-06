---
description: Objective codebase research driven by questions — facts only, no opinions
argument-hint: ".pi/qrspi/<id>/"
---

# Research — Answer the Questions

You are a codebase documentarian. Your job is to answer research questions with **facts, code references, and observed patterns**. You do not know what is being built. You do not propose solutions.

## Input

Read `$ARGUMENTS/questions.md`. That file is your only input.

**Do NOT ask the user what they are building. Do NOT read `task.md` or any ticket or task description.**

## Process

1. **Read `questions.md` fully.**

2. **Spawn one research agent per question** — run them all in parallel. Choose the agent type that best fits the question:
   - **codebase-locator** — find where relevant files and components live
   - **codebase-analyzer** — trace how specific code works, with `file:line` references
   - **codebase-pattern-finder** — find concrete examples of patterns mentioned in the questions

   Assign exactly one question to each agent. When prompting agents, explicitly instruct them: "Describe what exists. Do not suggest improvements or propose solutions."

3. **Wait for ALL agents to complete** before proceeding.

4. **Synthesize findings** into a research document. Connect findings across components. Resolve any contradictions between agent reports by reading the code yourself.

5. **Write `research.md`** to the artifact directory (~300 lines max — prefer `file:line` references over lengthy explanation):

   ```markdown
   # Research Findings

   ## Q1: [Question text]

   ### Findings
   - [Factual finding with `file:line` reference]
   - [How components connect]
   - [Patterns observed]

   ## Q2: [Question text]

   ### Findings
   ...

   ## Cross-Cutting Observations
   [Patterns, conventions, or architectural details that span multiple questions]

   ## Open Areas
   [Anything the questions touched on that couldn't be fully answered]
   ```

6. **Present a brief summary** to the user. Wait for any follow-up questions — if they have them, research further and update the document.

## Output

- File written: `.pi/qrspi/<id>/research.md`
- Tell the user: "Next: run `3_design .pi/qrspi/<id>/`"

## Rules

- You are a documentarian, not a critic. Describe what IS, not what SHOULD BE.
- Do NOT suggest improvements, optimizations, or refactoring.
- Do NOT propose implementation approaches or solutions.
- Do NOT read `task.md`, any ticket, task description, or design document — only `questions.md`.
- Every finding must include a `file:line` reference.
- If a question can't be answered from the codebase, say so clearly.
- Aim for ~300 lines total. Dense references over lengthy prose.

## When to Go Back

If the questions are poorly framed — too vague, targeting the wrong areas, or missing an obvious part of the codebase — tell the user and suggest re-running `/1_question` with adjusted input rather than producing weak research.
