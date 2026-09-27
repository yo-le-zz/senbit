mod build_cmd;
mod common;
mod getlinux;
mod getutil;
mod install_env;
mod run_vm;
mod steps;

use std::path::PathBuf;
use std::process::ExitCode;

use colored::Colorize;

use build_cmd::BuildTarget;
use common::Paths;

const USAGE: &str = "\
Senbit build tooling - Rust port of scripts/*.sh

Usage:
  senbit [--root <dir>] <command> [options]

Commands:
  build [target]     Compile Senbit (kernel, busybox, util-linux, parted,
                      rust, tools, rootfs, iso). target defaults to 'all'.
                      Equivalent of scripts/build.sh [target].
  run [--new] [--debug]
                      Build Senbit and launch it in QEMU.
                      Equivalent of scripts/run.sh.
  install             Install host build dependencies (apt) and fetch the
                      pinned Linux kernel. Equivalent of scripts/install.sh.
  getlinux            Fetch/update Linux kernel sources to the pinned
                      version. Equivalent of scripts/getlinux.sh.
  getutil             Fetch/update util-linux sources to the pinned
                      version. Equivalent of scripts/getutil.sh.

Options:
  --root <dir>        Project root (default: auto-detected, else
                       $SENBIT_ROOT, else the current directory).
  -h, --help           Show this help.
  -V, --version        Show version.
";

fn print_usage() {
    print!("{USAGE}");
}

fn fail(msg: impl AsRef<str>) -> ExitCode {
    eprintln!("{} {}", "Error:".red().bold(), msg.as_ref());
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // --root peut apparaitre n'importe ou avant la sous-commande.
    let mut root: Option<PathBuf> = None;
    let mut rest: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                match args.get(i) {
                    Some(v) => root = Some(PathBuf::from(v)),
                    None => return fail("--root requires a value"),
                }
            }
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("senbit {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            other => rest.push(other.to_string()),
        }
        i += 1;
    }

    if rest.is_empty() {
        print_usage();
        return ExitCode::FAILURE;
    }

    let paths = match Paths::resolve(root) {
        Ok(p) => p,
        Err(e) => return fail(e.to_string()),
    };

    let command = rest[0].as_str();
    let command_args = &rest[1..];

    let result = match command {
        "build" => {
            let target_str = command_args.first().map(|s| s.as_str()).unwrap_or("all");
            match BuildTarget::parse(target_str) {
                Ok(target) => build_cmd::run(&paths, target),
                Err(e) => return fail(e.to_string()),
            }
        }
        "run" => {
            let new_vm = command_args.iter().any(|a| a == "--new");
            let debug = command_args.iter().any(|a| a == "--debug");
            let known: [&str; 2] = ["--new", "--debug"];
            if let Some(unknown) = command_args.iter().find(|a| !known.contains(&a.as_str())) {
                return fail(format!(
                    "unknown argument: {unknown}\n\nUsage:\n  senbit run\n  senbit run --new\n  senbit run --debug\n  senbit run --new --debug"
                ));
            }
            run_vm::run(&paths, new_vm, debug)
        }
        "install" => install_env::run(&paths),
        "getlinux" => getlinux::run(&paths),
        "getutil" => getutil::run(&paths),
        "-h" | "--help" => {
            print_usage();
            return ExitCode::SUCCESS;
        }
        other => return fail(format!("unknown command: {other}\n\n{USAGE}")),
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
