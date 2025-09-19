# Development Guidelines

This document provides human-readable development guidelines for contributors.  
For GitHub Copilot-specific rules, see [`.github/copilot-instructions.md`](.github/copilot-instructions.md).

---

## General Principles
- Write **clear, maintainable, idiomatic Rust** code.
- Avoid unnecessary complexity—favor simple, well-factored functions.
- Use descriptive function and variable names; no cryptic abbreviations.
- Code should compile cleanly with **no warnings** and pass **all tests**.
- Prefer `PathBuf` for paths instead of raw `String`s.
- Debug output should always start with `DEBUG:`.

---

## Code Style
- Use the `?` operator for error propagation.
- Prefer `.expect("clear message")` over `.unwrap()` when a panic is acceptable.
- Derive common traits (`Debug`, `Clone`, `PartialEq`) where appropriate.
- Use structured error enums (with `thiserror`) instead of panics for recoverable errors.
- Document public items and complex functions with `///` doc comments.
- Inline `//` comments should explain **why** code is written in a certain way.
- Place comments next to the function or block they describe, not at call sites.

---

## Error Handling
- Always include context in error messages.
- Use a **standardized error reporting function** for consistency.
- Only auto-fix scenarios that are deterministic and safe. Otherwise, report a descriptive error.

---

## Testing
- Every new function should include a unit test.
- New features and bug fixes must include test coverage.
- Use `assert_cmd` for CLI testing and `mockito` for HTTP mocking.
- Mark long-running or external-dependency tests with `#[ignore]`.
- Avoid time-based fragility by using mocks (`TimeProvider`).

---

## GitHub Actions / CI
- Favor **reusable workflows** over copy-paste jobs.
- Avoid running duplicate jobs for `push` and `pull_request`.
- Always set `concurrency` in workflows to prevent overlapping runs.
- Use `Swatinem/rust-cache` for build caching.
- Use consistent naming: `release_build_test.yaml`, `release_upload.yaml`, `release_publish.yaml`.

---

## Commit Messages
We use the **[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)** standard.

- Messages must be **all lowercase**.
- Valid types are: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`.
- An optional scope may be added in parentheses: `feat(config): …`.
- Subject should be short and imperative.
- Body (optional) can explain *why* the change was made.
- Breaking changes and issue references belong in the footer.

### Good Examples
```
feat(config): support pathbuf for config path
fix(parser): handle empty lines in csv import
ci(workflow): add concurrency to prevent overlapping runs
```

### Bad Examples
```
Update files
Fix stuff
Refactor
```

---

## Pull Requests
- Include a summary and rationale for changes.
- Keep PRs small and focused.
- Link related issues.
- Ensure tests and documentation are updated.
- Run `clippy` and `rustfmt` before submission.

---

## Contributor Checklist
- [ ] Code follows idiomatic Rust practices.
- [ ] No `unwrap()` calls (use `expect` with clear messages).
- [ ] Errors are consistent and descriptive.
- [ ] Tests exist for all new functions, features, and fixes.
- [ ] Documentation (`///` and `//`) is present and accurate.
- [ ] Commit messages follow **Conventional Commits**.
- [ ] CI workflows pass cleanly and efficiently.

---

