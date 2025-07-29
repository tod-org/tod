# Usage

<!--toc:start-->
- [Usage](#usage)
  - [Discovering the commands](#discovering-the-commands)
  - [Usage Examples](#usage-examples)
  - [Shell script examples](#shell-script-examples)
    - [Sort, schedule, prioritize, and process tasks](#sort-schedule-prioritize-and-process-tasks)
  - [Update Tod](#update-tod)
  - [How task priority is determined](#how-task-priority-is-determined)
<!--toc:end-->

## Discovering the commands

```bash
> tod -h

An unofficial Todoist command line client

Usage: tod [OPTIONS] <COMMAND>

Commands:
  project  (p) Commands that change projects
  task     (t) Commands for individual tasks
  list     (l) Commands for multiple tasks
  config   (c) Commands around configuration and the app
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose          Display additional debug info while processing
  -c, --config <CONFIG>  Absolute path of configuration. Defaults to $XDG_CONFIG_HOME/tod.cfg
  -h, --help             Print help
  -V, --version          Print version
  ```

And also use it to dig into subcommands

```bash
> tod task -h

Commands for individual tasks

Usage: tod task <COMMAND>

Commands:
  quick-add  (q) Create a new task using NLP
  create     (c) Create a new task (without NLP)
  edit       (e) Edit an existing task's content
  next       (n) Get the next task by priority
  complete   (o) Complete the last task fetched with the next command
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## Usage Examples

```bash
# Quickly create a task
tod task quick-add --content Buy more milk today

# Quickly create a task with aliases
tod t q -c Buy more milk today

# You can use Todoist syntax with the quickadd (q) command
# See https://todoist.com/help/articles/use-task-quick-add-in-todoist-va4Lhpzz for more details
tod t q -c Buy more milk today // with a description

# Set a reminder
tod t q -c Buy more milk today ! today 2pm

# creates a task named "Clean my room" due on Tuesday at 1pm, with Priority of 2
tod t q -c Clean my room on tuesday at 1pm p2

# creates a task in the eBay project, an errands label, priority of 2, due tomorrow.
tod t q -c Ship UPS Package #eBay @errands p2 tomorrow

## Other Usage Examples

# Create a new task (you will be prompted for content and project)
tod task create

# Create a task in a project
tod task create --content "Write more rust" --project code

# Import your projects
tod project import

# Import all projects in Todoist into Tod
tod project import -a

# Get the next task for a project
tod task next

# Comment on the current (next) task
tod task comment

# Go through tasks with an interactive prompt, completing them in order of importance one at a time.
tod list process

# Complete the last "next task" and get another
tod task complete && tod task next

# Get all tasks for work
tod list view --project work

# Get all tasks in three groupings, overdue, today, and tomorrow
tod list view --filter overdue,today,tom

# Generate shell completions for fish
tod shell completions fish > ~/.config/fish/completions/tod.fish

# Label all tasks with no label either physical or digital
tod list label --filter "no label" --label physical --label digital

```

## Shell script examples

### Sort, schedule, prioritize, and process tasks

```bash
  echo "" && \
  echo "=== EMPTYING INBOX ===" && \
  tod project empty --project inbox && \
  echo "" && \
  echo "=== SCHEDULING DIGITAL ===" && \
  tod list schedule --project digital && \
  echo "" && \
  echo "=== SCHEDULING PHYSICAL ===" && \
  tod list schedule --project physical && \
  echo "" && \
  echo "=== PRIORITIZING DIGITAL ===" && \
  tod list prioritize --project digital && \
  echo "" && \
  echo "=== PRIORITIZING PHYSICAL ===" && \
  tod list prioritize --project physical
  echo "" && \
  echo "=== PROCESSING DIGITAL ===" && \
  tod list process --project digital && \
  echo "" && \
  echo "=== PROCESSING PHYSICAL ===" && \
  tod list process --project physical;
```

## How task priority is determined

See [Sort_value](https://github.com/tod-org/tod/blob/main/docs/configuration.md#sort_value)

## Update Tod

```bash
tod config check-version
```

## Update Tod (no prompt)

```bash
tod config check-version --force 
```

Tod will attempt to detect the installed method (homebrew, cargo, scoop, etc) and auto-update if possible.
