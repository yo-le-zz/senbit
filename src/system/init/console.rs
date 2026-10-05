//! Console font, loaded as early as possible so the installer and the boot
//! messages use it. The font is console state kept by the kernel: it stays
//! active after senbit-init hands over to systemd.

const FONT_CANDIDATES: [&str; 4] = [
    "/usr/share/consolefonts/Uni3-Terminus16.psf.gz",
    "/usr/share/consolefonts/Lat15-Terminus16.psf.gz",
    "/usr/share/consolefonts/ter-v16n.psf.gz",
    "/usr/share/consolefonts/default.psf.gz",
];

/// In VGA text mode (BIOS) the console uses the VGA ROM font, not the
/// kernel's: a PSF font must be loaded with `setfont` (BusyBox applet).
/// If no font is found, the default one is kept.
pub fn load_font() {
    for path in FONT_CANDIDATES {
        if !std::path::Path::new(path).exists() {
            continue;
        }

        // PID 1 has no controlling terminal, so BusyBox setfont cannot open
        // its default /dev/tty. Target the console explicitly with -C.
        for tty in ["/dev/tty0", "/dev/console"] {
            match std::process::Command::new("setfont")
                .args(["-C", tty, path])
                .status()
            {
                Ok(status) if status.success() => return,
                Ok(status) => {
                    eprintln!("setfont -C {tty} {path} failed: {status}");
                }
                Err(e) => {
                    eprintln!("cannot run setfont: {e}");
                    return;
                }
            }
        }

        return;
    }

    eprintln!("no console font found in /usr/share/consolefonts");
}
