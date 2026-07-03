use clap::{Parser, Subcommand};
use std::path::Path;
use walkdir::WalkDir;

use crate::{
    config::Config,
    errors::Error,
    filters, input,
    lists::{self, Flag},
    projects,
    tasks::SortOrder,
};

#[derive(Subcommand, Debug, Clone)]
pub enum ListCommands {
    #[clap(alias = "v")]
    /// (v) View a list of tasks
    View(View),

    #[clap(alias = "c")]
    /// (c) Complete a list of tasks one by one in priority order
    Process(Process),

    #[clap(alias = "z")]
    /// (z) Give every task a priority
    Prioritize(Prioritize),

    #[clap(alias = "r")]
    /// (r) Assign reminders to tasks that do not already have a reminder
    Remind(Remind),

    #[clap(alias = "t")]
    /// (t) Give every task a date, time, and duration
    Timebox(Timebox),

    #[clap(alias = "l")]
    /// (l) Iterate through tasks and apply labels from defined choices. Use label flag once per label to choose from.
    Label(Label),

    #[clap(alias = "s")]
    /// (s) Assign dates to all tasks individually
    Schedule(Schedule),

    #[clap(alias = "d")]
    /// (d) Assign deadlines to all non-recurring tasks without deadlines individually
    Deadline(Deadline),

    #[clap(alias = "i")]
    /// (i) Create tasks from a text file, one per line using natural language. Skips empty lines.
    Import(Import),
}

#[derive(Parser, Debug, Clone)]
pub struct View {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Datetime,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Process {
    #[arg(short, long)]
    /// Complete all tasks that are due today or undated in a project individually in priority order
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Timebox {
    #[arg(short, long)]
    /// Timebox all tasks without durations
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. It does not filter out tasks with durations unless specified in the filter. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Prioritize {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Remind {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Label {
    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short = 'l', long = "label")]
    /// Labels to select from, if left blank this will be fetched from API
    labels: Vec<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Schedule {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(short, long, default_value_t = false)]
    /// Don't re-schedule recurring tasks that are overdue
    skip_recurring: bool,

    #[arg(short, long, default_value_t = false)]
    /// Only schedule overdue tasks
    overdue: bool,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Deadline {
    #[arg(short, long)]
    /// The project containing the tasks
    project: Option<String>,

    #[arg(short, long)]
    /// The filter containing the tasks. Can add multiple filters separated by commas.
    filter: Option<String>,

    #[arg(
        short = 't',
        long,
        default_value_t = SortOrder::Value,
        default_missing_value = "value",
        num_args = 0..=1
    )]
    /// Choose how results should be sorted
    sort: SortOrder,
}

#[derive(Parser, Debug, Clone)]
pub struct Import {
    #[arg(short, long)]
    /// The file or directory to fuzzy find in
    path: Option<String>,
}
pub async fn view(config: &mut Config, args: &View) -> Result<String, Error> {
    let View {
        project,
        filter,
        sort,
    } = args;

    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), config).await?;
    lists::view(config, flag, sort).await
}

pub async fn label(config: Config, args: &Label) -> Result<String, Error> {
    let Label {
        filter,
        project,
        labels,
        sort,
    } = args;
    let labels = super::maybe_fetch_labels(&config, labels).await?;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::label(&config, flag, &labels, sort).await
}

pub async fn process(config: Config, args: &Process) -> Result<String, Error> {
    let Process {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::process(&config, flag, sort).await
}

pub async fn timebox(config: Config, args: &Timebox) -> Result<String, Error> {
    let Timebox {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::timebox(&config, flag, sort).await
}

pub async fn prioritize(config: Config, args: &Prioritize) -> Result<String, Error> {
    let Prioritize {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::prioritize(&config, flag, sort).await
}

pub async fn remind(config: Config, args: &Remind) -> Result<String, Error> {
    let Remind {
        project,
        filter,
        sort,
    } = args;
    let flag =
        super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await?;
    lists::remind(&config, flag, sort).await
}
pub async fn import(config: Config, args: &Import) -> Result<String, Error> {
    let Import { path } = args;
    let path = super::fetch_string(path.as_deref(), &config, input::PATH)?;
    let file_path = select_file(path, &config)?;
    lists::import(&config, &file_path).await
}

fn select_file(path_or_file: String, config: &Config) -> Result<String, Error> {
    let path = Path::new(&path_or_file);
    if Path::is_dir(path) {
        let mut options = WalkDir::new(path_or_file)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(is_md_file)
            .map(|e| {
                e.path()
                    .to_str()
                    .expect("Could not make str out of DirEntry")
                    .to_string()
            })
            .collect::<Vec<String>>();
        options.sort();
        options.dedup();
        let path = input::select("Select file to process", options, config.mock_select)?;

        Ok(path)
    } else if Path::is_file(path) {
        Ok(path_or_file)
    } else {
        Err(Error {
            source: "select_file".to_string(),
            message: format!("{path_or_file} is neither a file nor a directory"),
        })
    }
}

fn is_md_file(entry: &walkdir::DirEntry) -> bool {
    std::path::Path::new(entry.file_name().to_str().unwrap_or_default())
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

pub async fn schedule(config: Config, args: &Schedule) -> Result<String, Error> {
    let Schedule {
        project,
        filter,
        skip_recurring,
        overdue,
        sort,
    } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Filter(filter) => filters::schedule(&config, &filter, sort).await,
        Flag::Project(project) => {
            let task_filter = if *overdue {
                projects::TaskFilter::Overdue
            } else {
                projects::TaskFilter::Unscheduled
            };

            projects::schedule(&config, &project, task_filter, *skip_recurring, sort).await
        }
    }
}

pub async fn deadline(config: Config, args: &Deadline) -> Result<String, Error> {
    let Deadline {
        project,
        filter,
        sort,
    } = args;
    match super::fetch_project_or_filter(project.as_deref(), filter.as_deref(), &config).await? {
        Flag::Filter(filter) => filters::deadline(&config, &filter, sort).await,
        Flag::Project(project) => projects::deadline(&config, &project, sort).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_sort_without_value_uses_configured_sort() {
        let args = View::try_parse_from(["tod", "--sort"]).expect("--sort should be valid");
        assert_eq!(args.sort.to_string(), "value");
    }

    #[test]
    fn view_without_sort_keeps_datetime_default() {
        let args = View::try_parse_from(["tod"]).expect("view arguments should be valid");
        assert_eq!(args.sort.to_string(), "datetime");
    }
}
