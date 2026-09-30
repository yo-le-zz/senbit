use crate::logln;
use colored::Colorize;
use crate::utils::clear::clear_screen_ansi;

pub fn splash(version: &str, username: &str) {
    clear_screen_ansi();
    logln!(
        "{}",
        format!("Welcome to Senbit {}", version)
            .cyan()
            .bold()
    );

    logln!(
        "{}",
        format!("Hello, {}!", username)
            .green()
            .bold()
    );
}