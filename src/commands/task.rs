use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub sub: Option<TaskCmd>,
}

#[derive(Subcommand, Debug)]
pub enum TaskCmd {
    /// List tasks (issues), optionally filtered.
    List,
    /// Show one task by number.
    Show { number: u64 },
    /// Create a new task.
    New { title: String },
    /// Move a task to in-progress.
    Start { number: u64 },
    /// Mark a task blocked.
    Block { number: u64 },
    /// Mark a task done (closes the issue).
    Done { number: u64 },
    /// Comment on a task.
    Comment { number: u64, body: String },
}

pub fn run(_args: TaskArgs) -> anyhow::Result<i32> {
    super::not_yet("task", "P6")
}
