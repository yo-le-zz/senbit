//! Linux console handling: screen, controlling terminal, palette, font and
//! shell prompt.

use std::io::Write;

// Linux console ioctls (linux/kd.h)
const KDGETMODE: u32 = 0x4B3B;
const KDSKBMODE: u32 = 0x4B45;
const K_UNICODE: libc::c_int = 0x03;

// ============================================================
// Colors
// ============================================================
//
// The Linux console (VGA / framebuffer) does NOT support truecolor
// (ESC[38;2;r;g;bm): it maps every color to the nearest of its 16 palette
// entries. What we can do is REPROGRAM that palette (ESC ] P n RRGGBB) and
// then use plain ANSI codes.
//
// Entries 8, 9, 11, 12 and 13 are redefined. `.bold()` on the Linux console
// selects the bright variant (bold green = entry 10, bold cyan = entry 14),
// so entries 10 and 14 are left untouched.
// On a real terminal (serial port, SSH...), truecolor is kept.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Palette,
}

#[derive(Clone, Copy)]
struct Tint {
    rgb: (u8, u8, u8),
    slot: u8, // palette entry 8..=15
}

const BRACKET: Tint = Tint { rgb: (120, 120, 120), slot: 8 };
const USER: Tint = Tint { rgb: (200, 89, 9), slot: 9 };
const HOST: Tint = Tint { rgb: (148, 62, 56), slot: 11 };
const PATH: Tint = Tint { rgb: (56, 148, 116), slot: 12 };
const SYMBOL: Tint = Tint { rgb: (148, 96, 56), slot: 13 };

const PALETTE: [Tint; 5] = [BRACKET, USER, HOST, PATH, SYMBOL];

fn paint(text: &str, tint: Tint, bold: bool, mode: ColorMode) -> String {
    match mode {
        ColorMode::TrueColor => {
            let (r, g, b) = tint.rgb;
            let weight = if bold { "1;" } else { "" };
            format!("\x1b[{weight}38;2;{r};{g};{b}m{text}\x1b[0m")
        }
        ColorMode::Palette => {
            format!("\x1b[{}m{}\x1b[0m", 90 + tint.slot - 8, text)
        }
    }
}

// ============================================================
// Screen
// ============================================================

pub fn clear() {
    print!("\x1b[2J\x1b[1;1H");
    let _ = std::io::stdout().flush();
}

/// Makes /dev/console (our stdin) the controlling terminal of this process.
/// Best effort: it fails harmlessly if we already own it.
pub fn acquire_controlling_tty() {
    unsafe {
        libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 1);
    }
}

fn is_linux_vt() -> bool {
    let mut mode: libc::c_int = 0;
    unsafe { libc::ioctl(libc::STDOUT_FILENO, KDGETMODE as _, &mut mode) == 0 }
}

/// Prepares the console before a session and returns the color mode to use.
/// On a Linux console: UTF-8 mode, Unicode keyboard, custom palette.
pub fn prepare() -> ColorMode {
    if !is_linux_vt() {
        return ColorMode::TrueColor;
    }

    let mut out = std::io::stdout();

    // UTF-8 console (equivalent of `unicode_start`)
    let _ = out.write_all(b"\x1b%G");
    unsafe {
        libc::ioctl(libc::STDIN_FILENO, KDSKBMODE as _, K_UNICODE);
    }

    // Custom palette: ESC ] P <hex index> <RRGGBB>
    for tint in PALETTE {
        let (r, g, b) = tint.rgb;
        let _ = write!(out, "\x1b]P{:X}{:02X}{:02X}{:02X}", tint.slot, r, g, b);
    }
    let _ = out.flush();

    load_font();

    ColorMode::Palette
}

// ============================================================
// Console font
// ============================================================
//
// In VGA text mode (BIOS), the console uses the VGA ROM font, not the
// kernel's: a PSF font must be loaded with `setfont` (BusyBox applet).
// If no font is found, the default one is kept. The font is normally
// already loaded by senbit-init early in the boot; this only makes sure it
// is still active.

const FONT_CANDIDATES: [&str; 4] = [
    "/usr/share/consolefonts/Uni3-Terminus16.psf.gz",
    "/usr/share/consolefonts/Lat15-Terminus16.psf.gz",
    "/usr/share/consolefonts/ter-v16n.psf.gz",
    "/usr/share/consolefonts/default.psf.gz",
];

fn load_font() {
    let Some(path) = FONT_CANDIDATES
        .iter()
        .copied()
        .find(|p| std::path::Path::new(p).exists())
    else {
        return;
    };

    for tty in ["/dev/tty0", "/dev/console"] {
        match std::process::Command::new("setfont")
            .args(["-C", tty, path])
            .status()
        {
            Ok(status) if status.success() => return,
            Ok(status) => log_warn!("setfont -C {tty} {path} failed: {status}"),
            Err(error) => {
                log_warn!("cannot run setfont: {error}");
                return;
            }
        }
    }
}

pub fn term_for(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Palette => "linux",
        ColorMode::TrueColor => "xterm-256color",
    }
}

// ============================================================
// Prompt
// ============================================================

/// Builds PS1. The path (`\w`) and the symbol (`\$`) are left to ash so they
/// update on every `cd`.
pub fn build_prompt(mode: ColorMode, user: &str, hostname: &str) -> String {
    format!(
        "{}{}{} {}{}{} ",
        paint("[", BRACKET, false, mode),
        paint(user, USER, true, mode),
        paint(&format!("@{hostname}"), HOST, true, mode),
        paint("\\w", PATH, false, mode),
        paint("]", BRACKET, false, mode),
        paint("\\$", SYMBOL, true, mode),
    )
}
