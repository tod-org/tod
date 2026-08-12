# Research Questions

## Context

The `Config` struct (`src/config/mod.rs`) has several fields used only during testing. The serialization/deserialization behavior of these fields — some with custom deserializers and `skip_serializing` — affects what gets written to user config files on disk. The `input.rs` module uses `cfg!(test)` guards and takes mock parameters that flow in from the Config struct. The HTTP layer in `src/todoist/request.rs` and `src/cargo.rs` uses conditional-compilation-based URL overrides. Understanding the full data flow, serialization chain, and all call sites is essential before making changes.

## Questions

### 1. How does the HTTP base URL dispatch work in `src/todoist/request.rs` and `src/cargo.rs`?

Trace the complete flow of `get_base_url()` (line 233) and `get_latest_version()` (line 36) — specifically how `cfg!(test)` gates interact with `config.mock_url` and the standalone `mock_url: Option<String>` parameter. What happens at runtime outside tests vs inside tests when `mock_url` is `None`? What happens if a production config file on disk already has a `"mock_url"` key?

### 2. How do the `cfg!(test)` branches in `src/input.rs` use the mock parameters?

Trace how `select_with_cursor_index()` (line 253), `string()` (line 181), `multi_select()` (line 278), and `datetime()` (line 100) consume `mock_selects: &Mutex<Vec<usize>>` and `mock_string: Option<String>`. Where do these parameters originate, how are they threaded from `Config` fields into every input call site, and what happens if they are `None` or an empty vec?

### 3. What is the complete serialization/deserialization lifecycle of the Config struct's mock-related fields?

Map the serde attributes and behavior for `mock_url` (line 107), `mock_string` (line 108), and `mock_select` (lines 110-114). How does `deserialize_mock_select()` (line 58) achieve backward compatibility? What does `#[serde(default)]` supply for each field type when a key is missing? What exactly happens when `Config::save()` is called while `mock_url` or `mock_string` hold non-`None` values?

### 4. How does `Config::edit_interactive()` destructure and use the mock fields?

In `src/config/mod.rs` around line 590, `edit_interactive()` destructures `mock_select`, `mock_string`, and `mock_url` into underscore bindings but then passes `mock_select` to `input::bool()` calls. Trace which input functions it invokes and where `mock_select` is the only mock value used. What becomes of `mock_string` and `mock_url` during interactive editing?

### 5. What is the full inventory of non-test code that accesses `config.mock_select`, `config.mock_string`, and `config.mock_url`?

Identify every non-test code site where these fields are read from a Config reference — excluding test modules, builder methods, and test fixtures. Focus on: `src/tasks/mod.rs`, `src/projects.rs`, `src/lists.rs`, `src/filters.rs`, `src/sections.rs`, `src/commands/mod.rs`, `src/commands/task_commands.rs`, `src/commands/list_commands.rs`, `src/commands/section_commands.rs`, `src/commands/project_commands.rs`, `src/commands/label_commands.rs`, and `src/config/mod.rs`.

### 6. How do the test builder methods `with_mock_url()`, `with_mock_string()`, and `mock_select()` on Config work, and which test files use them?

The Config test impl block (around line 796) defines `with_mock_url(self, url: String)`, `with_mock_string(self, string: &str)`, `mock_select(self, index: usize)`, and `mock_selects(self, selects: Vec<usize>)`. How many test call sites use each method, and in which files? Are there tests that rely on these methods being chainable in specific orders?

### 7. How does `src/config/file.rs` handle the `mock_select` field during config save and load?

In `src/config/file.rs` around line 75, `Config::save()` clones `mock_select` into the config before writing. Trace why this clone exists, what serialization path `mock_select` takes (given `skip_serializing`), and how `mock_url`/`mock_string` flow through the same save path.
