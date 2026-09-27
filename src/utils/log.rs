// src/utils/log.rs

use std::fs::OpenOptions;
use std::io::{self, Write};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::utils::log::log(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {
        $crate::utils::log::logln(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! elog {
    ($($arg:tt)*) => {
        $crate::utils::log::elog(&format!($($arg)*));
    };
}

#[macro_export]
macro_rules! elogln {
    ($($arg:tt)*) => {
        $crate::utils::log::elogln(&format!($($arg)*));
    };
}

pub fn log(message: &str) {
    write_stdout(message);
}

pub fn logln(message: &str) {
    write_stdout(&format!("{}\n", message));
}

pub fn elog(message: &str) {
    write_stderr(message);
}

pub fn elogln(message: &str) {
    write_stderr(&format!("{}\n", message));
}

fn write_stdout(message: &str) {
    print!("{}", message);
    let _ = io::stdout().flush();

    write_serial(message);
}

fn write_stderr(message: &str) {
    eprint!("{}", message);
    let _ = io::stderr().flush();

    write_serial(message);
}

fn write_serial(message: &str) {
    if let Ok(mut serial) = OpenOptions::new()
        .write(true)
        .open("/dev/ttyS0")
    {
        let _ = serial.write_all(message.as_bytes());
        let _ = serial.flush();
    }
}