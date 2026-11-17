//! Linux console handling: screen, controlling terminal, palette, font and
//! shell prompt.

use std::io::Write;

// Linux console ioctls (linux/kd.h)
const KDGETMODE: u32 = 0x4B3B;
const KDSKBMODE: u32 = 0x4B45;
const K_UNICODE: libc::c_int = 0x03;
const KDGKBLED: u32 = 0x4B64;
const KDSKBLED: u32 = 0x4B65;
const KDSETLED: u32 = 0x4B32;
/// Flag NumLock of the keyboard LED state.
const LED_NUM: libc::c_int = 0x02;
/// KDSETLED argument: let the LEDs follow the flags again.
const LED_SHOW_FLAGS: libc::c_ulong = 0xFF;

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

/// Colors `text`. With `bash` the escape sequences are wrapped in `\[` `\]`:
/// bash needs them to know that they take no room on screen, otherwise a long
/// command wraps at the wrong column. ash does not know them, so the plain
/// prompt (PS1) and the bash one (SENBIT_PS1_BASH) are two different strings.
fn paint(text: &str, tint: Tint, bold: bool, mode: ColorMode, bash: bool) -> String {
    let (open, close) = match mode {
        ColorMode::TrueColor => {
            let (r, g, b) = tint.rgb;
            let weight = if bold { "1;" } else { "" };
            (format!("\x1b[{weight}38;2;{r};{g};{b}m"), "\x1b[0m".to_string())
        }
        ColorMode::Palette => (
            format!("\x1b[{}m", 90 + tint.slot - 8),
            "\x1b[0m".to_string(),
        ),
    };

    if bash {
        format!("\\[{open}\\]{text}\\[{close}\\]")
    } else {
        format!("{open}{text}{close}")
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

/// Builds PS1. The path (`\w`) and the symbol (`\$`) are left to the shell so
/// they update on every `cd`.
pub fn build_prompt(mode: ColorMode, user: &str, hostname: &str) -> String {
    prompt(mode, user, hostname, false)
}

/// Same prompt for bash (escape sequences wrapped in `\[` `\]`).
pub fn build_prompt_bash(mode: ColorMode, user: &str, hostname: &str) -> String {
    prompt(mode, user, hostname, true)
}

fn prompt(mode: ColorMode, user: &str, hostname: &str, bash: bool) -> String {
    format!(
        "{}{}{} {}{}{} ",
        paint("[", BRACKET, false, mode, bash),
        paint(user, USER, true, mode, bash),
        paint(&format!("@{hostname}"), HOST, true, mode, bash),
        paint("\\w", PATH, false, mode, bash),
        paint("]", BRACKET, false, mode, bash),
        paint("\\$", SYMBOL, true, mode, bash),
    )
}

/// Turns NumLock on by default on the Linux console (VGA / framebuffer).
///
/// KDSKBLED sets the flags AND the defaults (applied to every virtual
/// terminal), KDSETLED then makes the keyboard lights follow them. A serial
/// console has no keyboard: the ioctl fails and nothing happens.
pub fn enable_numlock() {
    unsafe {
        let fd = libc::open(
            b"/dev/console\0".as_ptr() as *const libc::c_char,
            libc::O_RDWR | libc::O_NOCTTY,
        );
        if fd < 0 {
            return;
        }

        let mut flags: libc::c_char = 0;
        if libc::ioctl(fd, KDGKBLED as _, &mut flags as *mut libc::c_char) == 0 {
            let flags = flags as libc::c_int;
            let current = (flags & 0x07) | LED_NUM;
            let default = ((flags >> 4) & 0x07) | LED_NUM;
            libc::ioctl(fd, KDSKBLED as _, (current | (default << 4)) as libc::c_ulong);
            libc::ioctl(fd, KDSETLED as _, LED_SHOW_FLAGS);
        }

        libc::close(fd);
    }
}
