//! bootreport : le rapport de démarrage de Senbit.
//!
//! senbit-init n'arrête plus le démarrage quand une étape non critique échoue
//! (réseau absent, hostname, logs, clavier) : il l'enregistre dans
//! /run/senbit/boot-report, puis passe la main à systemd. Cette commande lit ce
//! rapport :
//!
//!   bootreport            récapitulatif complet : étapes réussies, avertissements,
//!                         unités systemd en échec
//!   bootreport --banner   encadré compact, affiché à chaque connexion par
//!                         /etc/profile.d/senbit-bootreport.sh (il reste à l'écran
//!                         jusqu'à un `clear`)
//!   bootreport --logs     journal de systemd pour ce démarrage (journalctl -b)
//!   bootreport --kernel   journal du noyau (dmesg)
//!   bootreport --failed   unités systemd en échec
//!   bootreport --history  rapports des démarrages précédents
//!   bootreport --ports    ce qui écoute sur le réseau, et quel processus
//!   bootreport --verify   fichiers des paquets Senbit modifiés depuis leur installation
//!
//! Le bandeau et le rapport indiquent aussi les mises à jour disponibles (fichier
//! /var/lib/senbit/updates-available, écrit chaque jour par senbit-update-check.timer
//! et après chaque `apt upgrade`) : `sudo apt upgrade`.
//!
//! Format du fichier (une ligne par étape, séparée par des tabulations) :
//!   statut <TAB> uptime_ms <TAB> étape <TAB> détail      (statut = ok | warn | failed)

use std::collections::HashMap;
use std::fs;
use std::io::IsTerminal;
use std::process::{Command, ExitCode, Stdio};

const REPORT_FILE: &str = "/run/senbit/boot-report";
const HISTORY_FILE: &str = "/var/log/senbit/boot-report.log";
const UPDATES_FILE: &str = "/var/lib/senbit/updates-available";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Failed,
}

#[derive(Debug, Clone)]
struct Entry {
    status: Status,
    uptime_ms: u64,
    step: String,
    detail: String,
}

fn parse_report(text: &str) -> Vec<Entry> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(4, '\t');
            let status = match fields.next()? {
                "ok" => Status::Ok,
                "warn" => Status::Warn,
                "failed" => Status::Failed,
                _ => return None,
            };
            let uptime_ms = fields.next()?.parse().ok()?;
            let step = fields.next()?.to_string();
            let detail = fields.next().unwrap_or("").to_string();
            Some(Entry {
                status,
                uptime_ms,
                step,
                detail,
            })
        })
        .collect()
}

/// Mises à jour disponibles (voir /usr/lib/senbit/update-check).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Updates {
    total: usize,
    senbit: usize,
    packages: Vec<String>,
}

/// Ligne 1 : nombre de paquets ; ligne 2 : dont composants de Senbit ; puis les paquets.
fn parse_updates(text: &str) -> Option<Updates> {
    let mut lines = text.lines();
    let total = lines.next()?.trim().parse().ok()?;
    let senbit = lines.next()?.trim().parse().ok()?;
    let packages = lines
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Some(Updates { total, senbit, packages })
}

fn updates_text(updates: &Updates) -> String {
    if updates.senbit > 0 {
        format!(
            "{} update(s) available, {} for Senbit itself: sudo apt upgrade",
            updates.total, updates.senbit
        )
    } else {
        format!("{} update(s) available: sudo apt upgrade", updates.total)
    }
}

fn read_updates() -> Option<Updates> {
    let path = std::env::var("SENBIT_UPDATES").unwrap_or_else(|_| UPDATES_FILE.to_string());
    let updates = parse_updates(&fs::read_to_string(path).ok()?)?;
    if updates.total == 0 { None } else { Some(updates) }
}

/// (ok, warn, failed)
fn count(entries: &[Entry]) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for entry in entries {
        match entry.status {
            Status::Ok => counts.0 += 1,
            Status::Warn => counts.1 += 1,
            Status::Failed => counts.2 += 1,
        }
    }
    counts
}

fn format_time(ms: u64) -> String {
    format!("{}.{:03}s", ms / 1000, ms % 1000)
}

/// Coupe `text` en lignes d'au plus `width` caractères (aux espaces).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let separator = if current.is_empty() { 0 } else { 1 };
        let needed = separator + word.chars().count();
        if !current.is_empty() && current.chars().count() + needed > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct Style {
    color: bool,
}

impl Style {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
    fn green(&self, t: &str) -> String {
        self.paint("32", t)
    }
    fn yellow(&self, t: &str) -> String {
        self.paint("33;1", t)
    }
    fn red(&self, t: &str) -> String {
        self.paint("31;1", t)
    }
    fn bold(&self, t: &str) -> String {
        self.paint("1", t)
    }
    fn dim(&self, t: &str) -> String {
        self.paint("2", t)
    }
}

fn marker(status: Status) -> &'static str {
    match status {
        Status::Ok => "✓",
        Status::Warn => "⚠",
        Status::Failed => "✗",
    }
}

fn colorize_status(style: &Style, status: Status, text: &str) -> String {
    match status {
        Status::Ok => style.green(text),
        Status::Warn => style.yellow(text),
        Status::Failed => style.red(text),
    }
}

/// Encadré affiché à la connexion.
fn render_banner(
    entries: &[Entry],
    failed_units: &[String],
    updates: Option<&Updates>,
    style: &Style,
    width: usize,
) -> String {
    let width = width.clamp(40, 100);
    let inner = width - 4; // "│ " + " │"
    let (ok, warn, failed) = count(entries);
    let units_failed = failed_units.len();
    let has_problem = warn > 0 || failed > 0 || units_failed > 0;

    let mut rows: Vec<(Status, String)> = Vec::new();
    rows.push((
        if has_problem { Status::Warn } else { Status::Ok },
        format!(
            "{} {} OK   {} {} warning(s)   {} {} failed",
            marker(Status::Ok),
            ok,
            marker(Status::Warn),
            warn,
            marker(Status::Failed),
            failed + usize::from(units_failed > 0)
        ),
    ));

    for entry in entries.iter().filter(|e| e.status != Status::Ok) {
        let text = if entry.detail.is_empty() {
            entry.step.clone()
        } else {
            format!("{}: {}", entry.step, entry.detail)
        };
        for (i, line) in wrap(&text, inner - 2).into_iter().enumerate() {
            let prefix = if i == 0 { format!("{} ", marker(entry.status)) } else { "  ".to_string() };
            rows.push((entry.status, format!("{prefix}{line}")));
        }
    }

    if units_failed > 0 {
        let text = format!("systemd: {} failed unit(s): {}", units_failed, failed_units.join(" "));
        for (i, line) in wrap(&text, inner - 2).into_iter().enumerate() {
            let prefix = if i == 0 { format!("{} ", marker(Status::Failed)) } else { "  ".to_string() };
            rows.push((Status::Failed, format!("{prefix}{line}")));
        }
    }

    if let Some(updates) = updates {
        // ASCII marker: the Linux console font has no arrow glyphs.
        for (i, line) in wrap(&updates_text(updates), inner - 2).into_iter().enumerate() {
            let prefix = if i == 0 { "^ ".to_string() } else { "  ".to_string() };
            rows.push((Status::Warn, format!("{prefix}{line}")));
        }
    }

    let title = " Boot report ";
    let top_color = if has_problem { Status::Warn } else { Status::Ok };
    let mut out = String::new();

    let top = format!("┌─{title}{}┐", "─".repeat(width - 3 - title.chars().count()));
    out.push_str(&colorize_status(style, top_color, &top));
    out.push('\n');

    for (status, text) in &rows {
        let pad = inner.saturating_sub(text.chars().count());
        let line = format!("{text}{}", " ".repeat(pad));
        out.push_str(&colorize_status(style, top_color, "│ "));
        out.push_str(&colorize_status(style, *status, &line));
        out.push_str(&colorize_status(style, top_color, " │"));
        out.push('\n');
    }

    let bottom = format!("└{}┘", "─".repeat(width - 2));
    out.push_str(&colorize_status(style, top_color, &bottom));
    out.push('\n');

    out.push_str(&style.dim(&format!(
        "  bootreport: details | bootreport --logs: systemd logs of this boot | `clear` hides this box\n"
    )));
    out
}

/// Rapport complet.
fn render_full(
    entries: &[Entry],
    failed_units: &[String],
    updates: Option<&Updates>,
    style: &Style,
) -> String {
    let (ok, warn, failed) = count(entries);
    let mut out = String::new();

    out.push_str(&style.bold("Boot report"));
    out.push('\n');
    out.push_str(&"═".repeat(11));
    out.push('\n');

    for entry in entries {
        let line = format!(
            "{} {:<18} {:>9}  {}",
            marker(entry.status),
            entry.step,
            format_time(entry.uptime_ms),
            entry.detail
        );
        out.push_str(&colorize_status(style, entry.status, &line));
        out.push('\n');
    }

    out.push('\n');
    out.push_str(&format!(
        "{}  {}  {}\n",
        style.green(&format!("{} OK", ok)),
        style.yellow(&format!("{} warning(s)", warn)),
        style.red(&format!("{} failed", failed))
    ));

    if failed_units.is_empty() {
        out.push_str(&style.dim("systemd: no failed unit (or systemctl not reachable as this user)\n"));
    } else {
        out.push_str(&style.red(&format!(
            "systemd: {} failed unit(s): {}\n",
            failed_units.len(),
            failed_units.join(" ")
        )));
        out.push_str("  details: systemctl status <unit>   logs: journalctl -u <unit>\n");
    }

    match updates {
        Some(updates) => {
            out.push_str(&style.yellow(&format!("updates: {}\n", updates_text(updates))));
            for package in updates.packages.iter().take(15) {
                out.push_str(&format!("  {package}\n"));
            }
            if updates.packages.len() > 15 {
                out.push_str(&format!("  ... and {} more\n", updates.packages.len() - 15));
            }
        }
        None => out.push_str(&style.dim("updates: none known (checked every day)\n")),
    }

    if warn > 0 || failed > 0 {
        out.push('\n');
        out.push_str("Problems are not blocking: the system started. To retry the network:\n");
        out.push_str("  ip addr                                      (the name of your network card)\n");
        out.push_str("  sudo systemctl restart senbit-dhcp@<card>    (and: ip route)\n");
        out.push_str("  sudo apt update                              (once the network is back)\n");
    }

    out.push('\n');
    out.push_str(&style.dim("bootreport --logs     systemd journal of this boot\n"));
    out.push_str(&style.dim("bootreport --kernel   kernel log\n"));
    out.push_str(&style.dim("bootreport --history  previous boots\n"));
    out
}

fn read_report() -> Option<Vec<Entry>> {
    let path = std::env::var("SENBIT_BOOT_REPORT").unwrap_or_else(|_| REPORT_FILE.to_string());
    let text = fs::read_to_string(path).ok()?;
    let entries = parse_report(&text);
    if entries.is_empty() { None } else { Some(entries) }
}

/// Unités systemd en échec (liste vide si systemctl est injoignable).
fn failed_units() -> Vec<String> {
    let output = Command::new("systemctl")
        .args(["--failed", "--no-legend", "--plain", "--no-pager"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .filter(|unit| unit.contains('.'))
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(78)
}

fn make_style() -> Style {
    let color = std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true);
    Style { color }
}

/// Lance une commande en lui laissant le terminal ; renvoie son code de sortie.
fn run(program: &str, args: &[&str]) -> i32 {
    match Command::new(program).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("bootreport: cannot run {program}: {e}");
            127
        }
    }
}

fn exit_with(code: i32) -> ExitCode {
    ExitCode::from(code.clamp(0, 255) as u8)
}

fn logs() -> ExitCode {
    let code = run("journalctl", &["-b", "--no-pager"]);
    if code != 0 {
        eprintln!("bootreport: journalctl failed (root or the systemd-journal group is needed): try `sudo bootreport --logs`");
    }
    exit_with(code)
}

fn history() -> ExitCode {
    match fs::read_to_string(HISTORY_FILE) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("bootreport: no history ({HISTORY_FILE})");
            ExitCode::FAILURE
        }
    }
}

/* ---------------------------------------------------------------- listening ports */

/// One socket that accepts connections (TCP) or receives datagrams (UDP).
#[derive(Debug, PartialEq)]
struct Listener {
    protocol: &'static str,
    address: String,
    port: u16,
    /// Reachable only from this machine (127.0.0.1, ::1).
    local_only: bool,
    inode: u64,
}

/// Address as written in /proc/net/{tcp,udp}{,6}: 8 hex digits (IPv4, bytes in memory order)
/// or 32 (IPv6). Returns the text to show and whether it is a loopback address.
fn parse_address(hex: &str) -> (String, bool) {
    match hex.len() {
        8 => match u32::from_str_radix(hex, 16) {
            Ok(value) => {
                let bytes = value.to_le_bytes();
                let text = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                let local = bytes[0] == 127;
                (text, local)
            }
            Err(_) => (hex.to_string(), false),
        },
        32 if hex.chars().all(|c| c == '0') => ("::".to_string(), false),
        32 if hex == "00000000000000000000000001000000" => ("::1".to_string(), true),
        32 => ("[ipv6]".to_string(), false),
        _ => (hex.to_string(), false),
    }
}

/// Listening sockets of one /proc/net file. `state` is the hex state that means "listening":
/// `0A` for TCP, `07` for a bound UDP socket.
fn parse_listeners(text: &str, protocol: &'static str, state: &str) -> Vec<Listener> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 || fields[3] != state {
                return None;
            }
            let (address_hex, port_hex) = fields[1].rsplit_once(':')?;
            let port = u16::from_str_radix(port_hex, 16).ok()?;
            let (address, local_only) = parse_address(address_hex);
            let inode = fields[9].parse().ok()?;
            Some(Listener { protocol, address, port, local_only, inode })
        })
        .collect()
}

/// Which process owns which socket: the inode of every `socket:[N]` open in /proc/<pid>/fd.
/// Other users' processes are only visible to root.
fn socket_owners() -> HashMap<u64, String> {
    let mut owners = HashMap::new();
    let Ok(processes) = fs::read_dir("/proc") else {
        return owners;
    };

    for process in processes.flatten() {
        let pid = process.file_name().to_string_lossy().to_string();
        if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Ok(descriptors) = fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        let name = fs::read_to_string(format!("/proc/{pid}/comm"))
            .unwrap_or_default()
            .trim()
            .to_string();

        for descriptor in descriptors.flatten() {
            let Ok(target) = fs::read_link(descriptor.path()) else {
                continue;
            };
            let target = target.to_string_lossy().to_string();
            let inode = target
                .strip_prefix("socket:[")
                .and_then(|rest| rest.strip_suffix(']'))
                .and_then(|number| number.parse::<u64>().ok());
            if let Some(inode) = inode {
                owners.entry(inode).or_insert_with(|| format!("{name} ({pid})"));
            }
        }
    }
    owners
}

/// Number of sockets reachable from the network, for the login banner. The DHCP client
/// (UDP 68) is not counted: every machine has it.
fn exposed_count() -> usize {
    let mut count = 0;
    for (path, protocol, state) in [
        ("/proc/net/tcp", "tcp", "0A"),
        ("/proc/net/tcp6", "tcp6", "0A"),
        ("/proc/net/udp", "udp", "07"),
        ("/proc/net/udp6", "udp6", "07"),
    ] {
        if let Ok(text) = fs::read_to_string(path) {
            count += parse_listeners(&text, protocol, state)
                .iter()
                .filter(|l| !l.local_only && !(l.protocol.starts_with("udp") && l.port == 68))
                .count();
        }
    }
    count
}

fn ports() -> ExitCode {
    let mut listeners = Vec::new();
    for (path, protocol, state) in [
        ("/proc/net/tcp", "tcp", "0A"),
        ("/proc/net/tcp6", "tcp6", "0A"),
        ("/proc/net/udp", "udp", "07"),
        ("/proc/net/udp6", "udp6", "07"),
    ] {
        if let Ok(text) = fs::read_to_string(path) {
            listeners.extend(parse_listeners(&text, protocol, state));
        }
    }

    if listeners.is_empty() {
        println!("Nothing is listening on the network.");
        return ExitCode::SUCCESS;
    }

    listeners.sort_by(|a, b| (a.port, a.protocol).cmp(&(b.port, b.protocol)));
    let owners = socket_owners();

    println!("{:<6} {:<26} {:<11} {}", "PROTO", "ADDRESS", "REACHABLE", "PROCESS");
    for listener in &listeners {
        let reachable = if listener.local_only { "this machine" } else { "network" };
        let process = owners
            .get(&listener.inode)
            .map(String::as_str)
            .unwrap_or("? (root sees every process)");
        println!(
            "{:<6} {:<26} {:<11} {}",
            listener.protocol,
            format!("{}:{}", listener.address, listener.port),
            reachable,
            process
        );
    }

    let exposed = listeners.iter().filter(|l| !l.local_only).count();
    println!();
    println!("{exposed} of {} listening socket(s) can be reached from the network.", listeners.len());
    ExitCode::SUCCESS
}

/* ---------------------------------------------------------------- file integrity */

/// A `dpkg --verify` line for a configuration file: `??5?????? c /etc/...`. The
/// administrator is allowed to change those.
fn is_conffile_line(line: &str) -> bool {
    line.split_whitespace().nth(1) == Some("c")
}

fn verify() -> ExitCode {
    let output = match Command::new("dpkg")
        .args(["--verify", "senbit-kernel", "senbit-userspace"])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!("bootreport: cannot run dpkg: {error}");
            return ExitCode::FAILURE;
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let changed: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty() && !is_conffile_line(line))
        .collect();

    for line in String::from_utf8_lossy(&output.stderr).lines() {
        eprintln!("dpkg: {line}");
    }

    if changed.is_empty() {
        println!("Senbit files: nothing was modified since the packages were installed.");
        return ExitCode::SUCCESS;
    }

    println!("Senbit files modified since the packages were installed:");
    for line in &changed {
        println!("  {line}");
    }
    println!();
    println!("5 = content changed, M = mode, U/G = owner, missing = the file was removed.");
    println!("Reinstall them with: sudo apt install --reinstall senbit-userspace senbit-kernel");
    ExitCode::FAILURE
}

fn usage() {
    println!("usage: bootreport [--banner | --logs | --kernel | --failed | --history | --ports | --verify]");
    println!();
    println!("  (no option)  full boot report: what worked, what failed");
    println!("  --banner     compact box (shown at login)");
    println!("  --logs       systemd journal of this boot (journalctl -b)");
    println!("  --kernel     kernel log (dmesg)");
    println!("  --failed     failed systemd units");
    println!("  --history    reports of the previous boots");
    println!("  --ports      what listens on the network, and which process");
    println!("  --verify     Senbit files modified since the packages were installed");
}

#[path = "../../_shared/hook.rs"]
mod hook;

fn main() -> ExitCode {
    hook::install();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let style = make_style();

    match args.first().map(String::as_str) {
        None | Some("--full") => match read_report() {
            Some(entries) => {
                print!("{}", render_full(&entries, &failed_units(), read_updates().as_ref(), &style));
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("bootreport: no boot report ({REPORT_FILE}): this system was not started by senbit-init");
                ExitCode::FAILURE
            }
        },
        Some("--banner") => {
            // Silent when there is nothing to show: it runs at every login.
            if let Some(entries) = read_report() {
                print!(
                    "{}",
                    render_banner(&entries, &failed_units(), read_updates().as_ref(), &style, terminal_width())
                );

                // Computed once per boot by senbit-integrity.service.
                let modified: usize = fs::read_to_string("/run/senbit/integrity")
                    .ok()
                    .and_then(|text| text.trim().parse().ok())
                    .unwrap_or(0);
                if modified > 0 {
                    println!(
                        "{}",
                        style.dim(&format!(
                            "  {modified} Senbit file(s) modified since installation: bootreport --verify"
                        ))
                    );
                }

                let exposed = exposed_count();
                if exposed > 0 {
                    println!(
                        "{}",
                        style.dim(&format!(
                            "  {exposed} port(s) reachable from the network: bootreport --ports"
                        ))
                    );
                }
            }
            ExitCode::SUCCESS
        }
        Some("--logs") | Some("--log") => logs(),
        Some("--kernel") => exit_with(run("dmesg", &[])),
        Some("--failed") => exit_with(run("systemctl", &["--failed", "--no-pager"])),
        Some("--history") => history(),
        Some("--ports") => ports(),
        Some("--verify") => verify(),
        Some("-h") | Some("--help") => {
            usage();
            ExitCode::SUCCESS
        }
        Some("-V") | Some("--version") => {
            println!("bootreport {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("bootreport: unknown option `{other}`");
            usage();
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "ok\t1200\tRoot filesystem\t/dev/sda2 mounted\n\
                          ok\t1500\tHostname\tset\n\
                          warn\t3400\tNetwork\tno default route: the DHCP server did not answer\n";

    fn plain() -> Style {
        Style { color: false }
    }

    #[test]
    fn addresses_are_decoded() {
        assert_eq!(parse_address("0100007F"), ("127.0.0.1".to_string(), true));
        assert_eq!(parse_address("00000000"), ("0.0.0.0".to_string(), false));
        assert_eq!(parse_address("0201A8C0"), ("192.168.1.2".to_string(), false));
        assert_eq!(parse_address("00000000000000000000000001000000"), ("::1".to_string(), true));
    }

    #[test]
    fn listening_sockets_are_found() {
        let text = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
   0: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0\n\
   1: 0100007F:1F90 0100007F:D2A4 01 00000000:00000000 00:00000000 00000000     0        0 555 1 0000000000000000 100 0 0 10 0\n";
        let found = parse_listeners(text, "tcp", "0A");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].port, 22);
        assert_eq!(found[0].inode, 12345);
        assert!(!found[0].local_only);
    }

    #[test]
    fn configuration_files_are_not_reported_as_modified() {
        assert!(is_conffile_line("??5?????? c /etc/hosts"));
        assert!(!is_conffile_line("??5??????   /usr/sbin/rollback"));
        assert!(!is_conffile_line("missing     /usr/sbin/snapshot"));
    }

    #[test]
    fn parses_the_report() {
        let entries = parse_report(REPORT);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[2].status, Status::Warn);
        assert_eq!(entries[2].step, "Network");
        assert_eq!(entries[0].uptime_ms, 1200);
        assert_eq!(count(&entries), (2, 1, 0));
        assert!(parse_report("junk\nok\tx\ty\tz\n").is_empty());
    }

    #[test]
    fn times_are_formatted() {
        assert_eq!(format_time(1200), "1.200s");
        assert_eq!(format_time(59), "0.059s");
    }

    #[test]
    fn wrap_respects_the_width() {
        let lines = wrap("one two three four five six seven", 10);
        assert!(lines.iter().all(|l| l.chars().count() <= 10));
        assert_eq!(lines.join(" "), "one two three four five six seven");
        assert_eq!(wrap("", 10), vec![String::new()]);
    }

    #[test]
    fn banner_shows_the_warning_and_stays_in_a_box() {
        let entries = parse_report(REPORT);
        let banner = render_banner(&entries, &[], None, &plain(), 78);
        assert!(banner.contains("Boot report"));
        assert!(banner.contains("2 OK"));
        assert!(banner.contains("Network: no default route"));
        assert!(banner.contains("clear"));
        let box_lines: Vec<&str> = banner
            .lines()
            .filter(|l| l.starts_with('│') || l.starts_with('┌') || l.starts_with('└'))
            .collect();
        assert!(box_lines.len() >= 4);
        let widths: Vec<usize> = box_lines.iter().map(|l| l.chars().count()).collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "uneven box: {widths:?}");
    }

    #[test]
    fn banner_lists_failed_units() {
        let entries = parse_report("ok\t1\tA\tb\n");
        let banner = render_banner(&entries, &["foo.service".to_string()], None, &plain(), 78);
        assert!(banner.contains("foo.service"));
        assert!(banner.contains("1 failed"));
    }

    #[test]
    fn healthy_banner_has_no_problem_line() {
        let entries = parse_report("ok\t1\tA\tb\nok\t2\tB\tc\n");
        let banner = render_banner(&entries, &[], None, &plain(), 78);
        assert!(banner.contains("2 OK"));
        assert!(banner.contains("0 warning(s)"));
        assert!(!banner.contains("⚠ A") && !banner.contains("A:"));
    }

    #[test]
    fn full_report_lists_every_step_and_the_retry_hint() {
        let entries = parse_report(REPORT);
        let text = render_full(&entries, &[], None, &plain());
        assert!(text.contains("Root filesystem"));
        assert!(text.contains("Hostname"));
        assert!(text.contains("⚠ Network"));
        assert!(text.contains("not blocking"));
        assert!(text.contains("2 OK"));
    }

    #[test]
    fn updates_file_is_parsed() {
        let updates = parse_updates("3\n1\nsenbit-kernel\ncurl\nlibc-bin\n").unwrap();
        assert_eq!(updates.total, 3);
        assert_eq!(updates.senbit, 1);
        assert_eq!(updates.packages, vec!["senbit-kernel", "curl", "libc-bin"]);
        assert!(parse_updates("").is_none());
        assert!(parse_updates("x\n1\n").is_none());
        assert!(updates_text(&updates).contains("1 for Senbit itself"));
        assert_eq!(
            updates_text(&Updates { total: 2, senbit: 0, packages: vec![] }),
            "2 update(s) available: sudo apt upgrade"
        );
    }

    #[test]
    fn banner_and_report_announce_the_updates() {
        let entries = parse_report("ok\t1\tA\tb\n");
        let updates = Updates { total: 2, senbit: 1, packages: vec!["senbit-kernel".into(), "curl".into()] };

        let banner = render_banner(&entries, &[], Some(&updates), &plain(), 78);
        assert!(banner.contains("^ 2 update(s) available, 1 for Senbit itself"));
        let widths: Vec<usize> = banner
            .lines()
            .filter(|l| l.starts_with('│') || l.starts_with('┌') || l.starts_with('└'))
            .map(|l| l.chars().count())
            .collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "uneven box: {widths:?}");

        let full = render_full(&entries, &[], Some(&updates), &plain());
        assert!(full.contains("updates: 2 update(s) available"));
        assert!(full.contains("  senbit-kernel\n"));
        assert!(render_full(&entries, &[], None, &plain()).contains("updates: none known"));
    }

    #[test]
    fn colors_only_when_asked() {
        assert_eq!(Style { color: false }.red("x"), "x");
        assert!(Style { color: true }.red("x").contains("\x1b["));
    }
}
