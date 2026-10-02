// src/utils/panic.rs

use crate::kernel::alim;

use libc::{
    poll,
    pollfd,
    POLLIN,
};

use std::io::{
    self,
    Write,
};

use std::os::fd::AsRawFd;

use libc::{
    tcgetattr,
    tcsetattr,
    termios,
    TCSANOW,
    ECHO,
    ICANON,
    VMIN,
    VTIME,
};

/* ============================================================
   ANSI colors
============================================================ */

const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RESET: &str = "\x1b[0m";

/* ============================================================
   Wait for key
============================================================ */

fn wait_key_or_timeout(
    timeout_ms: i32,
) -> bool {
    let stdin =
        io::stdin();

    let fd =
        stdin.as_raw_fd();

    let mut pfd =
        pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };

    let ret =
        unsafe {
            poll(
                &mut pfd,
                1,
                timeout_ms,
            )
        };

    ret > 0
}

/* ============================================================
   Terminal raw mode
============================================================ */

fn set_terminal_raw() {
    let fd =
        io::stdin()
            .as_raw_fd();

    let mut term =
        unsafe {
            let mut t =
                std::mem::zeroed::<termios>();

            if tcgetattr(
                fd,
                &mut t,
            ) != 0 {
                return;
            }

            t
        };

    /*
     * Disable:
     * - canonical input
     * - input echo
     */

    term.c_lflag &=
        !(ICANON | ECHO);

    /*
     * Read input immediately.
     */

    term.c_cc[VMIN] = 0;
    term.c_cc[VTIME] = 0;

    unsafe {
        tcsetattr(
            fd,
            TCSANOW,
            &term,
        );
    }
}

/* ============================================================
   Panic countdown
============================================================ */

fn decompte() -> ! {
    set_terminal_raw();

    let mut remaining =
        5;

    loop {
        print!(
            "\r{}Shutdown in {} seconds, press any key for reboot{}",
            ANSI_YELLOW,
            remaining,
            ANSI_RESET,
        );

        let _ =
            io::stdout().flush();

        let key_pressed =
            wait_key_or_timeout(
                1000,
            );

        if key_pressed {
            alim::do_reboot();
        }

        if remaining == 0 {
            alim::do_power_off();
        }

        remaining -= 1;
    }
}

/* ============================================================
   Kernel panic
============================================================ */

pub fn kpanic_impl(
    msg: &str,
) -> ! {
    /*
     * Do NOT use the normal logging system here.
     *
     * The panic handler must remain functional even if
     * the logger, filesystem, or another system component
     * is unavailable.
     */

    eprintln!();

    eprintln!(
        "{}==============================",
        ANSI_RED,
    );

    eprintln!(
        "{}        SENBIT KERNEL PANIC",
        ANSI_RED,
    );

    eprintln!(
        "{}==============================",
        ANSI_RED,
    );

    eprintln!();

    eprintln!(
        "{}{}",
        ANSI_RED,
        msg,
    );

    eprintln!(
        "{}",
        ANSI_RESET,
    );

    eprintln!();

    decompte();
}