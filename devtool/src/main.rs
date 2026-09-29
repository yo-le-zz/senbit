mod build;
mod cache;
mod get;
mod gitutil;
mod install;
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

/// Senbit development tool — remplace scripts/build.sh, run.sh, install.sh,
/// getlinux.sh et getutil.sh.
#[derive(Parser)]
#[command(name = "devtool", version, about, long_about = None)]
struct Cli {
    /// Racine du projet Senbit (détectée automatiquement par défaut : le
    /// dossier parent de celui contenant l'exécutable `devtool`, sinon
    /// $SENBIT_ROOT, sinon le répertoire courant).
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile Senbit (noyau, BusyBox, util-linux, Parted, Rust, tools,
    /// rootfs, ISO). Équivalent de scripts/build.sh [cible].
    Build {
        #[arg(value_enum, default_value_t = Target::All)]
        target: Target,
    },

    /// Récupère ou met à jour les sources d'un composant à la version
    /// épinglée. Équivalent de scripts/getlinux.sh et scripts/getutil.sh.
    Get {
        #[arg(value_enum)]
        component: Component,
    },

    /// Installe les dépendances système de build. Équivalent de scripts/install.sh.
    Install,

    /// Construit Senbit puis le lance dans QEMU. Équivalent de scripts/run.sh.
    Run {
        /// Recrée entièrement la VM (supprime le disque et l'état existants).
        #[arg(long)]
        new: bool,
        /// Redirige la sortie série vers build/vm/qemu.log au lieu du terminal.
        #[arg(long)]
        debug: bool,
    },
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
        Commands::Build { target } => build::run(&paths, target),
        Commands::Get { component } => get::run(&paths, component),
        Commands::Install => install::run_install(&paths),
        Commands::Run { new, debug } => run_vm::run(&paths, new, debug),
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
