mod build;
mod cache;
mod config;
mod deps;
mod get;
mod gitutil;
mod host;
mod ovmf;
mod patches;
mod paths;
mod proc;
mod run_vm;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use colored::Colorize;

use build::Target;
use get::Component;
use paths::Paths;

/// Senbit development tool: one executable to check the machine, download the
/// pinned sources, build everything and boot it. Works on Fedora, Ubuntu /
/// Debian and Arch; every path is relative to the project root.
#[derive(Parser)]
#[command(name = "devtool", version, about, long_about = None)]
struct Cli {
    /// Project root (default: found by walking up from the `devtool`
    /// executable, then from the current directory; or $SENBIT_ROOT).
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check the build machine (distribution, tools, libraries) without
    /// changing anything, and print the exact install command.
    Doctor,

    /// Install the missing build dependencies with the distribution's package
    /// manager (apt, dnf or pacman) and the Rust musl target.
    Install,

    /// Download the sources at their PINNED versions (config/<name>/version).
    Get {
        #[arg(value_enum, default_value_t = Component::All)]
        component: Component,
    },

    /// Build Senbit (kernel, BusyBox, util-linux, Parted, systemd, apt, Rust,
    /// commands, tools, rootfs, ISO).
    Build {
        #[arg(value_enum, default_value_t = Target::All)]
        target: Target,

        /// Number of make/cargo jobs (default: detected CPU count).
        #[arg(short = 'j', long)]
        jobs: Option<usize>,

        /// Clear the smart-rebuild cache first: everything is rebuilt.
        #[arg(long)]
        force: bool,
    },

    /// List the Senbit native commands (commands/<name>/) or regenerate
    /// commands/INDEX.md. Add a command with ./newcmds.sh <name>.
    #[command(name = "commands")]
    Cmd {
        #[command(subcommand)]
        action: Option<CmdAction>,
    },

    /// Build Senbit, then boot it in QEMU.
    Run {
        /// Recreate the VM from scratch (deletes the disk and its state).
        #[arg(long)]
        new: bool,
        /// Write the whole boot (kernel, init, systemd) to build/vm/qemu.log.
        #[arg(long)]
        debug: bool,
        /// Only rebuild the Rust userspace, tools, rootfs and ISO.
        #[arg(long)]
        fast: bool,
        /// Clear the smart-rebuild cache before building.
        #[arg(long)]
        force: bool,
        /// Number of make/cargo jobs (default: detected CPU count).
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
    },
}

#[derive(Subcommand)]
enum CmdAction {
    /// List every detected command (default).
    List,
    /// Regenerate commands/INDEX.md without building.
    Index,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let paths = match Paths::resolve(cli.root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{} {e}", "Error:".red().bold());
            return ExitCode::FAILURE;
        }
    };

    let result = match cli.command {
        Commands::Build {
            target,
            jobs,
            force,
        } => build::run(&paths, target, jobs, force),
        Commands::Doctor => deps::doctor(&paths),
        Commands::Install => deps::install(&paths),
        Commands::Get { component } => get::run_get(&paths, component),
        Commands::Cmd { action } => match action.unwrap_or(CmdAction::List) {
            CmdAction::List => build::commands::list(&paths),
            CmdAction::Index => build::commands::index(&paths),
        },
        Commands::Run {
            new,
            debug,
            fast,
            force,
            jobs,
        } => run_vm::run(&paths, new, debug, fast, force, jobs),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!();
            eprintln!("{} {e:#}", "Error:".red().bold());
            ExitCode::FAILURE
        }
    }
}
