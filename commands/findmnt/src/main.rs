//! findmnt : liste les systèmes de fichiers montés (/proc/self/mountinfo).
//!
//! Commande de util-linux que Senbit n'embarque pas (il n'a que fdisk/sfdisk
//! statiques) : cette version couvre l'usage courant.
//!
//!   findmnt                     arbre des montages
//!   findmnt -l                  liste à plat
//!   findmnt /home               le montage dont la cible ou la source est /home
//!   findmnt -T /var/log/x       le système de fichiers qui contient ce fichier
//!   findmnt -t ext4,vfat        seulement ces types
//!   findmnt --real              sans les systèmes de fichiers virtuels (proc, sysfs...)
//!   findmnt -o TARGET,SOURCE,FSTYPE,OPTIONS,ID,PARENT,MAJ:MIN,FSROOT
//!   findmnt -n                  sans en-tête
//!
//! Codes de sortie : 0 trouvé, 1 rien ne correspond, 2 erreur d'usage.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

const MOUNTINFO: &str = "/proc/self/mountinfo";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mount {
    id: u32,
    parent: u32,
    major_minor: String,
    root: String,
    target: String,
    vfs_options: String,
    fstype: String,
    source: String,
    fs_options: String,
}

/// mountinfo écrit les espaces, tabulations, retours à la ligne et `\` des
/// chemins en octal (`\040`).
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4].iter().all(|b| (b'0'..=b'7').contains(b))
        {
            let value = (bytes[i + 1] - b'0') as u32 * 64
                + (bytes[i + 2] - b'0') as u32 * 8
                + (bytes[i + 3] - b'0') as u32;
            out.push(value as u8);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// `36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw,errors=continue`
fn parse_line(line: &str) -> Option<Mount> {
    let (left, right) = line.split_once(" - ")?;

    let left: Vec<&str> = left.split(' ').collect();
    if left.len() < 6 {
        return None;
    }

    let mut right = right.splitn(3, ' ');
    let fstype = right.next()?.to_string();
    let source = right.next()?.to_string();
    let fs_options = right.next().unwrap_or("").to_string();

    Some(Mount {
        id: left[0].parse().ok()?,
        parent: left[1].parse().ok()?,
        major_minor: left[2].to_string(),
        root: unescape(left[3]),
        target: unescape(left[4]),
        vfs_options: left[5].to_string(),
        fstype,
        source: unescape(&source),
        fs_options,
    })
}

fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines().filter_map(parse_line).collect()
}

/// Options du montage puis du système de fichiers, sans doublon (`rw` est dans les deux).
fn merge_options(vfs: &str, fs: &str) -> String {
    let mut seen = BTreeSet::new();
    let mut out: Vec<&str> = Vec::new();
    for option in vfs.split(',').chain(fs.split(',')) {
        if !option.is_empty() && seen.insert(option) {
            out.push(option);
        }
    }
    out.join(",")
}

/// `source[/racine]` pour un bind mount, comme util-linux.
fn source_display(m: &Mount) -> String {
    if m.root != "/" && m.source.starts_with('/') {
        format!("{}[{}]", m.source, m.root)
    } else {
        m.source.clone()
    }
}

fn column_value(m: &Mount, name: &str) -> Option<String> {
    Some(match name {
        "TARGET" => m.target.clone(),
        "SOURCE" => source_display(m),
        "FSTYPE" => m.fstype.clone(),
        "OPTIONS" => merge_options(&m.vfs_options, &m.fs_options),
        "VFS-OPTIONS" => m.vfs_options.clone(),
        "FS-OPTIONS" => m.fs_options.clone(),
        "ID" => m.id.to_string(),
        "PARENT" => m.parent.to_string(),
        "MAJ:MIN" => m.major_minor.clone(),
        "FSROOT" => m.root.clone(),
        _ => return None,
    })
}

const PSEUDO_FS: &[&str] = &[
    "autofs", "bpf", "binfmt_misc", "cgroup", "cgroup2", "configfs", "debugfs", "devpts",
    "devtmpfs", "efivarfs", "fusectl", "hugetlbfs", "mqueue", "proc", "pstore", "ramfs",
    "rpc_pipefs", "securityfs", "selinuxfs", "sysfs", "tmpfs", "tracefs",
];

/// Le montage qui contient `path` : la cible la plus longue dont `path` est un
/// descendant ; à cible égale, le plus récent (id le plus grand).
fn mount_for_path<'a>(mounts: &'a [Mount], path: &str) -> Option<&'a Mount> {
    let path = Path::new(path);
    mounts
        .iter()
        .filter(|m| path.starts_with(&m.target))
        .max_by_key(|m| (m.target.len(), m.id))
}

struct Glyphs {
    tee: &'static str,
    last: &'static str,
    vert: &'static str,
    blank: &'static str,
}

fn glyphs(utf8: bool) -> Glyphs {
    if utf8 {
        Glyphs { tee: "├─", last: "└─", vert: "│ ", blank: "  " }
    } else {
        Glyphs { tee: "|-", last: "`-", vert: "| ", blank: "  " }
    }
}

fn visit(
    mounts: &[Mount],
    children: &BTreeMap<u32, Vec<usize>>,
    index: usize,
    prefix: &str,
    branch: &str,
    is_last: bool,
    g: &Glyphs,
    out: &mut Vec<(String, usize)>,
) {
    out.push((format!("{prefix}{branch}"), index));

    let child_prefix = if branch.is_empty() {
        prefix.to_string()
    } else if is_last {
        format!("{prefix}{}", g.blank)
    } else {
        format!("{prefix}{}", g.vert)
    };

    let kids = children.get(&mounts[index].id).cloned().unwrap_or_default();
    for (n, kid) in kids.iter().enumerate() {
        let last = n + 1 == kids.len();
        let own = if last { g.last } else { g.tee };
        visit(mounts, children, *kid, &child_prefix, own, last, g, out);
    }
}

/// Lignes de l'arbre : (préfixe de branche, index du montage).
fn tree_order(mounts: &[Mount], utf8: bool) -> Vec<(String, usize)> {
    let ids: BTreeSet<u32> = mounts.iter().map(|m| m.id).collect();
    let mut children: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut roots: Vec<usize> = Vec::new();

    for (i, m) in mounts.iter().enumerate() {
        if m.parent == m.id || !ids.contains(&m.parent) {
            roots.push(i);
        } else {
            children.entry(m.parent).or_default().push(i);
        }
    }
    for kids in children.values_mut() {
        kids.sort_by_key(|&i| mounts[i].id);
    }
    roots.sort_by_key(|&i| mounts[i].id);

    let g = glyphs(utf8);
    let mut out = Vec::new();
    for root in roots {
        visit(mounts, &children, root, "", "", true, &g, &mut out);
    }
    out
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
}

/// Tableau : colonne 1 = cible (avec l'arbre si `tree`).
fn render(
    mounts: &[Mount],
    rows: &[(String, usize)],
    columns: &[String],
    headings: bool,
    style: &Style,
) -> String {
    let mut table: Vec<Vec<String>> = Vec::new();
    for (prefix, index) in rows {
        let m = &mounts[*index];
        let mut line = Vec::new();
        for (c, name) in columns.iter().enumerate() {
            let mut value = column_value(m, name).unwrap_or_default();
            if c == 0 {
                value = format!("{prefix}{value}");
            }
            line.push(value);
        }
        table.push(line);
    }

    let mut widths: Vec<usize> = columns.iter().map(|c| c.chars().count()).collect();
    for line in &table {
        for (i, cell) in line.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    let mut out = String::new();

    if headings {
        let mut parts = Vec::new();
        for (i, name) in columns.iter().enumerate() {
            let pad = widths[i] - name.chars().count();
            let cell = if i + 1 == columns.len() {
                name.clone()
            } else {
                format!("{name}{}", " ".repeat(pad))
            };
            parts.push(style.paint("1", &cell));
        }
        out.push_str(&parts.join(" "));
        out.push('\n');
    }

    for line in &table {
        let mut parts = Vec::new();
        for (i, cell) in line.iter().enumerate() {
            let pad = widths[i] - cell.chars().count();
            let text = if i + 1 == line.len() {
                cell.clone()
            } else {
                format!("{cell}{}", " ".repeat(pad))
            };
            // Target in green, like a path.
            parts.push(if i == 0 { style.paint("32", &text) } else { text });
        }
        out.push_str(parts.join(" ").trim_end());
        out.push('\n');
    }
    out
}

struct Options {
    list: bool,
    no_headings: bool,
    real: bool,
    types: Vec<String>,
    columns: Vec<String>,
    target_of: Option<String>,
    positional: Option<String>,
}

fn usage() {
    println!("usage: findmnt [-l] [-n] [--real] [-t types] [-o columns] [-T file | device | mountpoint]");
    println!("columns: TARGET SOURCE FSTYPE OPTIONS VFS-OPTIONS FS-OPTIONS ID PARENT MAJ:MIN FSROOT");
}

fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut o = Options {
        list: false,
        no_headings: false,
        real: false,
        types: Vec::new(),
        columns: ["TARGET", "SOURCE", "FSTYPE", "OPTIONS"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        target_of: None,
        positional: None,
    };

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;

        let mut value = |name: &str| -> Result<String, String> {
            let v = args
                .get(i)
                .cloned()
                .ok_or_else(|| format!("option {name} needs an argument"))?;
            i += 1;
            Ok(v)
        };

        match arg {
            "-h" | "--help" => {
                usage();
                return Ok(None);
            }
            "-l" | "--list" => o.list = true,
            "-n" | "--noheadings" => o.no_headings = true,
            "--real" => o.real = true,
            "-t" | "--types" => {
                o.types = value("-t")?.split(',').map(str::to_string).collect();
            }
            "-o" | "--output" => {
                o.columns = value("-o")?
                    .split(',')
                    .map(|c| c.trim().to_uppercase())
                    .filter(|c| !c.is_empty())
                    .collect();
            }
            "-T" | "--target" => o.target_of = Some(value("-T")?),
            // Accepted for compatibility, no effect.
            "-k" | "--kernel" | "-v" | "--nofsroot" | "-u" | "--submounts" | "-U" | "--uniq" => {}
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("unknown option `{other}`"));
            }
            other => {
                if o.positional.is_some() {
                    return Err("only one device or mountpoint can be given".to_string());
                }
                o.positional = Some(other.to_string());
            }
        }
    }

    for column in &o.columns {
        if column_value(
            &Mount {
                id: 0,
                parent: 0,
                major_minor: String::new(),
                root: String::new(),
                target: String::new(),
                vfs_options: String::new(),
                fstype: String::new(),
                source: String::new(),
                fs_options: String::new(),
            },
            column,
        )
        .is_none()
        {
            return Err(format!("unknown column: {column}"));
        }
    }

    Ok(Some(o))
}

fn utf8_locale() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .map(|v| {
            let v = v.to_lowercase();
            v.contains("utf-8") || v.contains("utf8")
        })
        .unwrap_or(false)
}

fn run(args: &[String]) -> ExitCode {
    let options = match parse_args(args) {
        Ok(Some(o)) => o,
        Ok(None) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("findmnt: {e}");
            usage();
            return ExitCode::from(2);
        }
    };

    let text = match fs::read_to_string(MOUNTINFO) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("findmnt: cannot read {MOUNTINFO}: {e}");
            return ExitCode::from(2);
        }
    };
    let all = parse_mountinfo(&text);

    let style = Style {
        color: std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true),
    };

    // Which mounts, and tree or list?
    let filtering = options.positional.is_some()
        || options.target_of.is_some()
        || !options.types.is_empty()
        || options.real;

    let mut selected: Vec<Mount> = all.clone();

    if let Some(path) = &options.target_of {
        let canonical = fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.clone());
        selected = mount_for_path(&all, &canonical).cloned().into_iter().collect();
    }
    if let Some(what) = &options.positional {
        selected.retain(|m| &m.target == what || &m.source == what || &source_display(m) == what);
    }
    if !options.types.is_empty() {
        selected.retain(|m| options.types.iter().any(|t| t == &m.fstype));
    }
    if options.real {
        selected.retain(|m| !PSEUDO_FS.contains(&m.fstype.as_str()));
    }

    if selected.is_empty() {
        return ExitCode::from(1);
    }

    let as_tree = !options.list && !filtering;
    let rows: Vec<(String, usize)> = if as_tree {
        tree_order(&selected, utf8_locale())
    } else {
        (0..selected.len()).map(|i| (String::new(), i)).collect()
    };

    print!(
        "{}",
        render(&selected, &rows, &options.columns, !options.no_headings, &style)
    );
    ExitCode::SUCCESS
}

#[path = "../../_shared/hook.rs"]
mod hook;

fn main() -> ExitCode {
    hook::install();
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = "\
24 1 8:2 / / rw,relatime shared:1 - ext4 /dev/sda2 rw
25 24 0:5 / /proc rw,nosuid,nodev,noexec,relatime shared:2 - proc proc rw
26 24 0:6 / /dev rw,nosuid shared:3 - devtmpfs devtmpfs rw,size=934176k
27 26 0:22 / /dev/shm rw,nosuid,nodev shared:4 - tmpfs tmpfs rw
28 24 8:1 / /boot\\040efi rw,relatime shared:5 - vfat /dev/sda1 rw,fmask=0022
29 24 8:2 /var/lib/docker /mnt/docker rw,relatime shared:6 - ext4 /dev/sda2 rw
";

    #[test]
    fn parses_mountinfo_lines() {
        let mounts = parse_mountinfo(INFO);
        assert_eq!(mounts.len(), 6);
        assert_eq!(mounts[0].target, "/");
        assert_eq!(mounts[0].source, "/dev/sda2");
        assert_eq!(mounts[0].fstype, "ext4");
        assert_eq!(mounts[0].parent, 1);
        // \040 = space
        assert_eq!(mounts[4].target, "/boot efi");
        assert!(parse_line("garbage").is_none());
    }

    #[test]
    fn octal_escapes() {
        assert_eq!(unescape("/a\\040b"), "/a b");
        assert_eq!(unescape("/a\\134b"), "/a\\b");
        assert_eq!(unescape("plain"), "plain");
        assert_eq!(unescape("end\\04"), "end\\04");
    }

    #[test]
    fn options_are_merged_without_duplicates() {
        assert_eq!(merge_options("rw,relatime", "rw,errors=continue"), "rw,relatime,errors=continue");
        assert_eq!(merge_options("rw", ""), "rw");
    }

    #[test]
    fn bind_mounts_show_their_root() {
        let mounts = parse_mountinfo(INFO);
        assert_eq!(source_display(&mounts[5]), "/dev/sda2[/var/lib/docker]");
        assert_eq!(source_display(&mounts[0]), "/dev/sda2");
    }

    #[test]
    fn tree_follows_the_parents() {
        let mounts = parse_mountinfo(INFO);
        let order = tree_order(&mounts, true);
        let labels: Vec<(String, &str)> = order
            .iter()
            .map(|(p, i)| (p.clone(), mounts[*i].target.as_str()))
            .collect();
        assert_eq!(labels[0], ("".to_string(), "/"));
        assert_eq!(labels[1], ("├─".to_string(), "/proc"));
        assert_eq!(labels[2], ("├─".to_string(), "/dev"));
        // /dev/shm is the child of /dev, which is not the last child of /
        assert_eq!(labels[3], ("│ └─".to_string(), "/dev/shm"));
        assert_eq!(labels[4], ("├─".to_string(), "/boot efi"));
        assert_eq!(labels[5], ("└─".to_string(), "/mnt/docker"));
    }

    #[test]
    fn ascii_tree_for_non_utf8_locales() {
        let mounts = parse_mountinfo(INFO);
        let order = tree_order(&mounts, false);
        assert_eq!(order[1].0, "|-");
        assert_eq!(order[5].0, "`-");
    }

    #[test]
    fn finds_the_mount_that_contains_a_path() {
        let mounts = parse_mountinfo(INFO);
        assert_eq!(mount_for_path(&mounts, "/dev/shm/x").unwrap().target, "/dev/shm");
        assert_eq!(mount_for_path(&mounts, "/etc/passwd").unwrap().target, "/");
        assert_eq!(mount_for_path(&mounts, "/devices").unwrap().target, "/");
    }

    #[test]
    fn table_has_headings_and_aligned_columns() {
        let mounts = parse_mountinfo(INFO);
        let rows: Vec<(String, usize)> = (0..mounts.len()).map(|i| (String::new(), i)).collect();
        let columns: Vec<String> = ["TARGET", "SOURCE", "FSTYPE"].iter().map(|s| s.to_string()).collect();
        let text = render(&mounts, &rows, &columns, true, &Style { color: false });
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("TARGET"));
        assert_eq!(lines.len(), 7);
        let source_col = lines[0].find("SOURCE").unwrap();
        assert_eq!(&lines[1][source_col..source_col + 9], "/dev/sda2");

        let no_head = render(&mounts, &rows, &columns, false, &Style { color: false });
        assert_eq!(no_head.lines().count(), 6);
    }

    #[test]
    fn arguments() {
        let args = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        let o = parse_args(&args(&["-l", "-n", "--real", "-t", "ext4,vfat", "-o", "target,fstype", "/boot"]))
            .unwrap()
            .unwrap();
        assert!(o.list && o.no_headings && o.real);
        assert_eq!(o.types, vec!["ext4", "vfat"]);
        assert_eq!(o.columns, vec!["TARGET", "FSTYPE"]);
        assert_eq!(o.positional.as_deref(), Some("/boot"));
        assert!(parse_args(&args(&["-o", "BOGUS"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
        assert!(parse_args(&args(&["-t"])).is_err());
        assert!(parse_args(&args(&["/a", "/b"])).is_err());
    }
}
