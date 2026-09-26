// src/utils/panic.rs

use colored::Colorize;
use crate::kernel::alim;

use libc::{poll, pollfd, POLLIN};
use std::io::{self, Write};
use std::os::fd::AsRawFd;

use libc::{tcgetattr, tcsetattr, termios, TCSANOW, ECHO, ICANON, VMIN, VTIME};

fn wait_key_or_timeout(timeout_ms: i32) -> bool {
    let stdin = io::stdin();
    let fd = stdin.as_raw_fd();

    let mut pfd = pollfd {
        fd,
        events: POLLIN,
        revents: 0,
    };

    let ret = unsafe { poll(&mut pfd, 1, timeout_ms) };

    // ret > 0 => il y a des données (une touche est pressée)
    ret > 0
}

fn set_terminal_raw() {
    use std::os::fd::AsRawFd;

    let fd = io::stdin().as_raw_fd();

    let mut term = unsafe {
        let mut t: termios = std::mem::zeroed();
        tcgetattr(fd, &mut t);
        t
    };

    // Désactiver canonical mode et echo
    term.c_lflag &= !(ICANON | ECHO);

    // Mode non-bloquant : VMIN=0, VTIME=0
    term.c_cc[VMIN] = 0;
    term.c_cc[VTIME] = 0;

    unsafe {
        tcsetattr(fd, TCSANOW, &term);
    }
}

fn decompte() -> ! {
    // Mettre le terminal en mode raw (touche par touche, sans echo)
    set_terminal_raw();

    let mut remaining = 5;

    loop {
        print!("\rShutdown in {} seconds, press any key for reboot", remaining);
        io::stdout().flush().unwrap();

        let key_pressed = wait_key_or_timeout(1000);

        if key_pressed {
            crate::kernel::alim::do_reboot();
        }

        if remaining == 0 {
            crate::kernel::alim::do_power_off();
        }

        remaining -= 1;
    }
}

pub fn kpanic_impl(msg: &str) -> ! {
    eprintln!("{}", msg.red());
    decompte();
}