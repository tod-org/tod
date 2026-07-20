# CI & Testing

This document gives a high-level overview of the automated checks that run
against every change to `tod`.

## Philosophy

Checks are layered by cost and how often they need to run:

- **Every push** gets a fast, single-OS smoke test so feedback is quick.
- **Every pull request** gets a fuller pass, including coverage reporting.
- **Release PRs** get the full cross-platform matrix plus a real end-to-end
  run against the live Todoist API — the most expensive and most realistic
  checks, reserved for the point right before code ships.

Independent of that pipeline, security and hygiene checks (secret
scanning, static analysis, commit message format, changelog consistency)
run on their own schedules and triggers.

## Everyday checks

| Workflow | Runs on | What it does |
| --- | --- | --- |
| **CI Push** | every push (except release-please branches) | Quick feedback loop: lint + a single-OS test run. No coverage upload, kept deliberately minimal. |
| **CI PR** | every pull request | Same lint suite as push, plus the test run reports results to Codecov. |
| **Codecov** | pushes touching Rust/config files | Runs the test suite under `cargo tarpaulin` and uploads coverage data to Codecov, independent of the PR-triggered report. |

Both CI Push and CI PR are built from two shared building blocks so the
same job definitions aren't duplicated:

- **Reusable / Lint** — `cargo check`, `cargo fmt --check`, `cargo clippy`
  (warnings treated as errors), and a scan for stray `TODO`/`FIXME`/`dbg!`
  markers left in the code.
- **Reusable / Test** — runs the test suite with `cargo nextest`, on
  whichever OS(es) the caller asks for.

## Release checks

| Workflow | Runs on | What it does |
| --- | --- | --- |
| **CI Release** | release PRs (opened by release-please) or manual trigger | Runs the *full* cross-platform test matrix (Linux, Linux/ARM, macOS, Windows) and, only for genuine release PRs, also triggers the Todoist E2E suite below. |
| **E2E Todoist Integration Tests** | manual trigger, or automatically as part of CI Release | Exercises `tod` against a real Todoist account rather than mocks — see [`ci.md`](./ci.md) for the full breakdown of what's covered (task/project lifecycle, config commands, sorting, filtering, labels, quick-add) and what's intentionally out of scope. |
| **Release Check** | pull requests targeting `main` | Confirms the version number in `CHANGELOG.md` matches `Cargo.toml`, so a release can't ship with mismatched versions. |
| **Dependabot Full CI** | Dependabot PRs | Runs the full cross-platform test matrix against dependency-bump PRs specifically, since a dependency update is exactly the kind of change most likely to break a single platform silently. |

## Security & hygiene

| Workflow | Runs on | What it does |
| --- | --- | --- |
| **gitleaks** | every push, every PR, and nightly | Scans the repository for accidentally committed secrets (API keys, tokens, credentials). |
| **CodeQL Analysis** | weekly schedule | Static analysis for security vulnerabilities across the codebase and GitHub Actions workflows themselves, plus a separate Clippy-based scan uploaded as security findings. |
| **Commit Lint** | every pull request | Enforces [Conventional Commits](https://www.conventionalcommits.org/) formatting on commit messages. |
| **Check PR Merge** | pull requests | Blocks merging if a PR is labeled `do not merge` or `WIP`. |

## Why a live-API E2E suite at all?

Everything above the E2E suite runs against mocked or unit-level behavior.
That's fast and cheap, but it can't catch problems that only show up
against Todoist's actual API: a CLI flag that's silently ignored, a filter
query that Todoist itself interprets differently than expected, or a sort
order that only looks right in a mock. The E2E suite trades speed for
realism — it's the one layer that would actually notice if `tod` stopped
working against real Todoist, which is why it's reserved for release time
rather than run on every push.
