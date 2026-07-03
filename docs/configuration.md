# Configuration

<!--toc:start-->
- [Configuration](#configuration)
  - [Location](#location)
  - [Values](#values)
    - [disable_links](#disable_links)
    - [last_version_check](#last_version_check)
    - [max_comment_length](#max_comment_length)
    - [next_id](#next_id)
    - [path](#path)
    - [natural_language_only](#natural_language_only)
    - [no_sections](#no_sections)
    - [projectsv1](#projectsv1)
    - [sort_order](#sort_order)
    - [spinners](#spinners)
    - [timeout](#timeout)
    - [timezone](#timezone)
    - [token](#token)
    - [timeprovider](#timeprovider)
    - [task_create_command](#task_create_command)
    - [task_comment_command](#task_comment_command)
    - [task_complete_command](#task_complete_command)
    - [verbose](#verbose)
<!--toc:end-->

If the config does not exist, Tod will prompt for your initial Todoist API token and create a default config with the following values:

``` json
{
  "bell_on_failure": true,
  "bell_on_success": false,
  "completed": null,
  "disable_links": false,
  "last_version_check": null,
  "max_comment_length": null,
  "mock_select": null,
  "mock_string": null,
  "mock_url": null,
  "natural_language_only": null,
  "next_id": null,
  "next_taskv1": null,
  "no_sections": null,
  "path": "See Location - Platform Specific",
  "projectsv1": [],
    "sort_value": {
    "deadline_days": 5,
    "deadline_value": 30,
    "no_due_date": 80,
    "not_recurring": 50,
    "now": 200,
    "overdue": 150,
    "priority_high": 4,
    "priority_low": 1,
    "priority_medium": 3,
    "priority_none": 2,
    "today": 100
  },
  "sort_order": [
    "priority:desc",
    "due_date:asc",
    "overdue:desc",
    "today:desc",
    "now:desc",
    "no_due_date:desc",
    "not_recurring:desc",
    "deadline:asc",
    "order:asc"
  ],
  "spinners": true,
  "timeout": null,
  "timezone": "",
  "token": "Your Todoist API Todken",
  "verbose": null
}
```

The Config can be deleted with `tod config reset` at any time, and it will be re-created upon next execution.

Run `tod config check` to validate the configuration file and optionally remove invalid values, such as old fields left behind by previous versions.

## Location

 Data is stored in JSON format in `$XDG_CONFIG_HOME/tod.cfg`. This defaults to:

- `~/.config/tod.cfg` on Linux
- `~/Library/Application Support/tod.cfg` on Mac
- No idea about Windows, sorry!

## Values

### bell_on_success

``` json
  type: boolean
  default: false
```

Triggers the terminal bell on successful completion of a command

### bell_on_failure

``` json
  type: boolean
  default: true
```

Triggers the terminal bell on an error

### disable_links

``` json
  type: boolean
  default: false
```

If true, disables OSC8 linking and just displays plain text

### last_version_check

``` json
  type: nullable string
  default: null
  possible_values: any string in format YYYY-MM-DD
```

Holds a string date, i.e. `"2023-08-30"` representing the last time crates.io was checked for the latest `tod` version. Tod will check crates.io a maximum of once per day.

### max_comment_length

``` json
  type: nullable positive integer
  default: null
  possible_values: Any positive integer or null
```

The maximum number of characters used as the starting point for shortening comments.
When comments exceed this value, output is shortened at the next clean boundary:
the next newline first, then the next terminal-width boundary, then the next
valid character boundary.

If not set, this is dynamically calculated at runtime based on terminal window size (using the `term_size` crate).

### next_id

``` json
  type: nullable string
  default: null
  possible values: null or any positive integer in string form
```

When `task next` is executed the ID is stored in this field. When `task complete` is run the field is set back to `null`

### path

``` json
  type: string
  default: $XDG_CONFIG_HOME/tod.cfg
  possible values: Any path
```

Location of the `tod` configuration file

### natural_language_only

``` json
  type: nullable boolean
  default: null
  possible values: null, true, or false
```

If true, the datetime selection in `project schedule` will go straight to natural language input.

### no_sections

``` json
  type: nullable boolean
  default: null
  possible values: null, true, or false
```

If true will not prompt for a section whenever possible

### projectsv1

```json
  type: Nullable array of objects
  default: []
  possible values: List of project objects from the Todoist API
```

Projects are stored locally in config to help save on API requests and speed up actions taken. Manage this with the `project` subcommands.

### sort_value

Deprecated in latest version, replaced with sort_order. Will be removed in future release.

  {
    "deadline_days": 5,
    "deadline_value": 30,
    "no_due_date": 80,
    "not_recurring": 50,
    "now": 200,
    "overdue": 150,
    "priority_high": 4,
    "priority_low": 1,
    "priority_medium": 3,
    "priority_none": 2,
    "today": 100
  },

### sort_order

List of sort rules used when sorting with the default `value` sort. Each rule uses `key:asc` or `key:desc`. Tod compares tasks only by the configured rules, in order. If tasks are equal after those comparisons, their order from the Todoist API is preserved.

New config files include this default order: `priority:desc`, `due_date:asc`, `overdue:desc`, `today:desc`, `now:desc`, `no_due_date:desc`, `not_recurring:desc`, `deadline:asc`, `order:asc`.

The direction may be omitted to use the key's default. For example, `priority` is equivalent to `priority:desc`:

``` json
  "sort_order": ["priority", "due_date:desc"]
```

Available keys: `priority`, `due_date`, `overdue`, `today`, `now`, `no_due_date`, `not_recurring`, `deadline`, and `order`. Available directions are `asc` and `desc`. `order` uses Todoist's task order (`child_order` in the current API).

Legacy configs that still contain `sort_value` will be accepted temporarily. Tod migrates the old numeric weights into a best-effort `sort_order` at load time and prints a warning that `sort_value` will be removed in a future version. To avoid the warning, replace `sort_value` with an explicit `sort_order` list.

### spinners

``` json
  type: nullable boolean
  default: null
  possible values: null, true, or false
```

Controls whether the spinner is displayed when an API call occurs. Useful for cases where the terminal output is captured. `null` is considered the same as `true`.

You can also use the environment variable `DISABLE_SPINNER` to turn them off.

```bash
  DISABLE_SPINNER=1 tod task create
```

### timeout

```json
  type: integer
  default: 30 (seconds)
  possible values: Any positive number in seconds
```

### timezone

```json
  type: string
  default: No default
  possible values: Any timezone string i.e. "Canada/Pacific"
```

You will be prompted for timezone on first run

### token

```json
  type: string
  default: No default
  possible values: Any valid token
```

You will be prompted for your [Todoist API token](https://todoist.com/prefs/integrations) on first run or if this is otherwise invalid/unset.

### timeprovider

```json
  type: string
  default: No default
  possible values: Enum of SystemTimeProvider or FixedTimeProvider
```

Used for dev/testing only to return fixed time (fixture) for use in test cases. Otherwise defaults to SystemTimeProvider in all other cases.

### task_comment_command

``` json
type: string
default: null
possible values: Any valid executable shell command (such as 'echo task commented')
```

Defaults to `null` (no command). The shell command spawned in the background after a task is commented. Only executes if set. Allows for custom integration with other scripts, code, sounds, or webhooks.

Command output is suppressed while the hook runs so it cannot interfere with terminal rendering. Tod discards `stdin` and `stdout`, captures `stderr`, and reports a shell command error at the end if the hook cannot be started or exits unsuccessfully. If the hook exits successfully, its output is discarded.

### task_create_command

``` json
type: string
default: null
possible values: Any valid executable shell command (such as 'echo task created')
```

Defaults to `null` (no command). The shell command spawned in the background after a task is added or created. Only executes if set, for both regular and quick-add task creation. Allows for custom integration with other scripts, code, sounds, or webhooks.

Command output is suppressed while the hook runs so it cannot interfere with terminal rendering. Tod discards `stdin` and `stdout`, captures `stderr`, and reports a shell command error at the end if the hook cannot be started or exits unsuccessfully. If the hook exits successfully, its output is discarded.

### task_complete_command

``` json
type: string
default: null
possible values: Any valid executable shell command (such as 'echo task completed')
```

Defaults to `null` (no command is run). The shell command spawned in the background after a task is completed. Only executes if set. Allows for custom integration with other scripts, code, sounds, or webhooks.

Command output is suppressed while the hook runs so it cannot interfere with terminal rendering. Tod discards `stdin` and `stdout`, captures `stderr`, and reports a shell command error at the end if the hook cannot be started or exits unsuccessfully. If the hook exits successfully, its output is discarded.

### task_exclude_regex

``` json
type: Regex in String form (JSON escaped)  
default: null
possible values: Any valid Regex expression
```

Defaults to `null` (no tasks excluded). This field must be a valid JSON-escaped regex string. Any tasks for which their title (`content`) matches will NOT be returned.

For example, this could be used to exclude uncompletable tasks ("^* ").

### comment_exclude_regex

``` json
type: Regex in String form (JSON escaped)  
default: null
possible values: Any valid Regex expression
```

Defaults to `null` (no comments excluded). This field must be a valid JSON-escaped regex value. Any comments for which their title (`content`) matches will NOT be returned.

### verbose

```json
  type: nullable boolean
  default: null
  possible values: null, true, or false
```

Outputs additional information in console to assist with debugging.
