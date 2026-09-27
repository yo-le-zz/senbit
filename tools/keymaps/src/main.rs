// tools/keymaps/src/main.rs

mod web;
mod keymaps;

use web::downloader;
use keymaps::keymap;


use colored::Colorize;

const CONFIG_PATH: &str = "tools/keymaps/config.toml";
const OUTPUT_PATH: &str = "build/tools/generated/keymaps";

fn main() {
    if let Err(error) = std::fs::create_dir_all(OUTPUT_PATH) {
        eprintln!(
            "{}: {}",
            "Failed to create output directory".red(),
            error
        );
        return;
    }

    if let Err(error) = downloader::download_all(CONFIG_PATH, OUTPUT_PATH) {
        eprintln!(
            "{}: {}",
            "Failed to download keymaps".red(),
            error
        );
        std::process::exit(1);
    }

    if let Err(error) = keymap::compile(CONFIG_PATH, OUTPUT_PATH) {
        eprintln!(
            "{}: {}",
            "Failed to compile keymaps".red(),
            error
        );
        std::process::exit(1);
    }

    println!("{}", "Keymaps compiled successfully!".green());
}