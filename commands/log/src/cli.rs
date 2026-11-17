use clap::{
    Args,
    Parser,
    Subcommand,
    ValueEnum,
};

#[derive(Parser, Debug)]
#[command(
    name = "log",
    version,
    about = "Senbit system log management utility",
    long_about = "Inspect, filter, follow, rotate and manage Senbit system logs."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(
        short = 'n',
        long = "lines",
        global = true,
        value_name = "N",
        help = "Maximum number of log lines to display"
    )]
    pub lines: Option<usize>,

    #[arg(
        long = "max-size",
        global = true,
        value_name = "SIZE",
        help = "Maximum amount of log data to read, e.g. 512K, 2M or 1G"
    )]
    pub max_size: Option<String>,

    #[arg(
        long = "since",
        global = true,
        value_name = "TIME",
        help = "Show entries after a timestamp or relative duration"
    )]
    pub since: Option<String>,

    #[arg(
        long = "until",
        global = true,
        value_name = "TIME",
        help = "Show entries before a timestamp or relative duration"
    )]
    pub until: Option<String>,

    #[arg(
        long = "level",
        global = true,
        value_name = "LEVEL",
        help = "Minimum log level"
    )]
    pub level: Option<Level>,

    #[arg(
        long = "source",
        global = true,
        value_name = "SOURCE",
        help = "Log source: all, persistent, runtime, system"
    )]
    pub source: Option<Source>,

    #[arg(
        long = "grep",
        global = true,
        value_name = "TEXT",
        help = "Only show entries containing TEXT"
    )]
    pub grep: Option<String>,

    #[arg(
        long = "regex",
        global = true,
        value_name = "REGEX",
        help = "Only show entries matching REGEX"
    )]
    pub regex: Option<String>,

    #[arg(
        long = "exclude",
        global = true,
        value_name = "TEXT",
        help = "Exclude entries containing TEXT"
    )]
    pub exclude: Vec<String>,

    #[arg(
        long = "exclude-module",
        global = true,
        value_name = "MODULE",
        help = "Exclude entries from a module"
    )]
    pub exclude_module: Vec<String>,

    #[arg(
        long = "exclude-level",
        global = true,
        value_name = "LEVEL",
        help = "Exclude a log level"
    )]
    pub exclude_level: Vec<Level>,

    #[arg(
        long = "json",
        global = true,
        help = "Output structured JSON"
    )]
    pub json: bool,

    #[arg(
        long = "raw",
        global = true,
        help = "Output raw log lines without formatting"
    )]
    pub raw: bool,

    #[arg(
        long = "no-color",
        global = true,
        help = "Disable colored output"
    )]
    pub no_color: bool,

    #[arg(
        long = "color",
        global = true,
        help = "Force colored output"
    )]
    pub color: bool,

    #[arg(
        short = 'q',
        long = "quiet",
        global = true,
        help = "Suppress informational output"
    )]
    pub quiet: bool,

    #[arg(
        short = 'v',
        long = "verbose",
        global = true,
        help = "Enable verbose output"
    )]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(
        about = "Show logs",
        alias = "view"
    )]
    Show,

    #[command(
        about = "Follow logs in real time",
        alias = "tail"
    )]
    Follow,

    #[command(
        about = "Show boot logs"
    )]
    Boot,

    #[command(
        about = "Show system logs"
    )]
    System,

    #[command(
        about = "Show only error logs"
    )]
    Errors,

    #[command(
        about = "Show only warning logs"
    )]
    Warnings,

    #[command(
        about = "Show kernel messages"
    )]
    Kernel(KernelArgs),

    #[command(
        about = "List available log sources"
    )]
    List,

    #[command(
        about = "Show log statistics"
    )]
    Stats,

    #[command(
        about = "Delete log files"
    )]
    Clear(ClearArgs),

    #[command(
        about = "Rotate log files"
    )]
    Rotate(RotateArgs),
}

#[derive(Args, Debug)]
pub struct KernelArgs {
    #[arg(
        short = 'n',
        long = "lines",
        value_name = "N",
        help = "Maximum number of kernel messages"
    )]
    pub lines: Option<usize>,
}

#[derive(Args, Debug)]
pub struct ClearArgs {
    #[arg(
        long = "runtime",
        help = "Clear only runtime logs"
    )]
    pub runtime: bool,

    #[arg(
        long = "persistent",
        help = "Clear only persistent logs"
    )]
    pub persistent: bool,

    #[arg(
        long = "all",
        help = "Clear all Senbit logs"
    )]
    pub all: bool,

    #[arg(
        short = 'f',
        long = "force",
        help = "Do not ask for confirmation"
    )]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct RotateArgs {
    #[arg(
        long = "runtime",
        help = "Rotate only runtime logs"
    )]
    pub runtime: bool,

    #[arg(
        long = "persistent",
        help = "Rotate only persistent logs"
    )]
    pub persistent: bool,

    #[arg(
        long = "all",
        help = "Rotate all Senbit logs"
    )]
    pub all: bool,

    #[arg(
        long = "keep",
        default_value_t = 3,
        value_name = "N",
        help = "Number of rotated files to keep"
    )]
    pub keep: usize,

    #[arg(
        long = "max-size",
        value_name = "SIZE",
        help = "Rotate only when the file reaches this size"
    )]
    pub max_size: Option<String>,

    #[arg(
        long = "force",
        help = "Rotate even if the size threshold was not reached"
    )]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    pub fn priority(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warn => 1,
            Self::Info => 2,
            Self::Debug => 3,
            Self::Trace => 4,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    ValueEnum,
)]
pub enum Source {
    All,
    Persistent,
    Runtime,
    System,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Persistent => "persistent",
            Self::Runtime => "runtime",
            Self::System => "system",
        }
    }
}