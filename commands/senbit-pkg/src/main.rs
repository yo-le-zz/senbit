//! senbit-pkg : le gestionnaire de paquets de Senbit.
//!
//! Format de paquet : le `.deb` de Debian (voir docs/PACKAGES.md). Senbit
//! réutilise apt et dpkg (résolution de dépendances, scripts de mainteneur,
//! base /var/lib/dpkg, signatures GPG des dépôts) ; senbit-pkg est l'interface
//! unique par-dessus :
//!
//!   senbit-pkg update | upgrade | install <paquet...> | remove | purge | autoremove | clean
//!   senbit-pkg search <terme> | show <paquet> | list | files <paquet> | owns <fichier>
//!   senbit-pkg repo list | add <nom> <uri> <suite> [composants...] --key <fichier> | remove <nom>
//!   senbit-pkg keys list | add <nom> <fichier>
//!
//! Les options après la commande sont transmises à apt (`senbit-pkg install -y curl`).
//! Les commandes qui modifient le système passent par sudo si on n'est pas root.

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const SOURCES_DIR: &str = "/etc/apt/sources.list.d";
const KEYRING_DIR: &str = "/etc/apt/keyrings";

/* ------------------------------------------------------------------ */
/* Fonctions pures (testées)                                          */
/* ------------------------------------------------------------------ */

/// Nom de dépôt / de clé : sert de nom de fichier, donc strict.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn valid_uri(uri: &str) -> bool {
    (uri.starts_with("http://") || uri.starts_with("https://") || uri.starts_with("file:/"))
        && !uri.contains(char::is_whitespace)
}

struct Repo<'a> {
    uri: &'a str,
    suite: &'a str,
    components: &'a [String],
    architecture: &'a str,
    /// Chemin de la clé (Signed-By) ; `None` = dépôt non signé.
    signed_by: Option<&'a str>,
}

/// Contenu d'un fichier `.sources` (format deb822).
fn render_sources(name: &str, repo: &Repo) -> String {
    let components = if repo.components.is_empty() {
        "main".to_string()
    } else {
        repo.components.join(" ")
    };

    let mut text = format!("# Senbit repository `{name}` (senbit-pkg repo add)\n");
    text.push_str("Types: deb\n");
    text.push_str(&format!("URIs: {}\n", repo.uri));
    text.push_str(&format!("Suites: {}\n", repo.suite));
    text.push_str(&format!("Components: {components}\n"));
    text.push_str(&format!("Architectures: {}\n", repo.architecture));
    match repo.signed_by {
        Some(key) => text.push_str(&format!("Signed-By: {key}\n")),
        None => text.push_str("Trusted: yes\n"),
    }
    text
}

/// (URIs, Suites) d'un fichier `.sources`, pour `repo list`.
fn summarize_sources(text: &str) -> (String, String) {
    let mut uris = String::new();
    let mut suites = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("URIs:") {
            uris = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Suites:") {
            suites = v.trim().to_string();
        }
    }
    (uris, suites)
}

/// Extension à garder pour la clé : apt lit les `.asc` (ASCII) et `.gpg` (binaire).
fn key_extension(path: &str) -> &'static str {
    if path.ends_with(".asc") || path.ends_with(".txt") {
        "asc"
    } else {
        "gpg"
    }
}

/// Commande réelle (programme + arguments de tête) d'une commande senbit-pkg.
fn apt_command(command: &str) -> Option<Vec<&'static str>> {
    Some(match command {
        "update" => vec!["apt-get", "update"],
        "upgrade" => vec!["apt-get", "upgrade"],
        "full-upgrade" | "dist-upgrade" => vec!["apt-get", "dist-upgrade"],
        "install" | "add" => vec!["apt-get", "install"],
        "reinstall" => vec!["apt-get", "install", "--reinstall"],
        "remove" | "uninstall" => vec!["apt-get", "remove"],
        "purge" => vec!["apt-get", "purge"],
        "autoremove" => vec!["apt-get", "autoremove"],
        "clean" => vec!["apt-get", "clean"],
        "fix" => vec!["apt-get", "install", "--fix-broken"],
        "search" => vec!["apt-cache", "search"],
        "show" | "info" => vec!["apt-cache", "show"],
        "policy" => vec!["apt-cache", "policy"],
        "depends" => vec!["apt-cache", "depends"],
        "rdepends" => vec!["apt-cache", "rdepends"],
        "files" => vec!["dpkg", "-L"],
        "owns" => vec!["dpkg", "-S"],
        "status" => vec!["dpkg", "-s"],
        "configure" => vec!["dpkg", "--configure", "-a"],
        "hold" => vec!["apt-mark", "hold"],
        "unhold" => vec!["apt-mark", "unhold"],
        _ => return None,
    })
}

/// Cette commande modifie-t-elle le système ?
fn needs_root(command: &str, args: &[String]) -> bool {
    match command {
        "update" | "upgrade" | "full-upgrade" | "dist-upgrade" | "install" | "add" | "reinstall"
        | "remove" | "uninstall" | "purge" | "autoremove" | "clean" | "fix" | "configure" | "hold"
        | "unhold" => true,
        "repo" => matches!(args.first().map(String::as_str), Some("add") | Some("remove")),
        "keys" => matches!(args.first().map(String::as_str), Some("add") | Some("remove")),
        _ => false,
    }
}

/* ------------------------------------------------------------------ */
/* Exécution                                                          */
/* ------------------------------------------------------------------ */

fn is_root() -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/proc/self").map(|m| m.uid() == 0).unwrap_or(false)
}

/// Remplace le processus par `program args...` ; ne revient que sur erreur.
fn exec(program: &str, args: &[String], interactive: bool) -> ExitCode {
    let mut command = Command::new(program);
    command.args(args);
    if !interactive {
        // Pas de question de debconf / needrestart / apt-listchanges : un script
        // ou un `senbit-pkg install postgresql` ne doit jamais rester bloqué.
        command.env("DEBIAN_FRONTEND", "noninteractive");
        command.env("APT_LISTCHANGES_FRONTEND", "none");
        command.env("NEEDRESTART_MODE", "a");
    }
    let error = command.exec();
    eprintln!("senbit-pkg: cannot run {program}: {error}");
    ExitCode::from(127)
}

/// Se relance sous sudo quand une commande qui modifie le système est lancée
/// par un utilisateur normal.
fn reexec_with_sudo(args: &[String]) -> ExitCode {
    let me = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("senbit-pkg"));
    let mut sudo_args = vec![me.to_string_lossy().to_string()];
    sudo_args.extend(args.iter().cloned());
    eprintln!("senbit-pkg: this needs root, running through sudo...");
    exec("sudo", &sudo_args, true)
}

/* ------------------------------------------------------------------ */
/* Dépôts et clés                                                     */
/* ------------------------------------------------------------------ */

fn repo_list() -> Result<(), String> {
    let dir = Path::new(SOURCES_DIR);
    let mut found = false;
    let mut entries: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    };
    entries.sort();

    for path in entries {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let Some(stem) = name.strip_suffix(".sources") else {
            continue;
        };
        let text = fs::read_to_string(&path).unwrap_or_default();
        let (uris, suites) = summarize_sources(&text);
        let signed = if text.contains("Signed-By:") { "signed" } else { "UNSIGNED" };
        println!("{stem:<20} {signed:<9} {uris} {suites}");
        found = true;
    }
    if !found {
        println!("No repository in {SOURCES_DIR}.");
    }
    Ok(())
}

struct RepoAdd {
    name: String,
    uri: String,
    suite: String,
    components: Vec<String>,
    key: Option<String>,
    allow_unsigned: bool,
    architecture: String,
}

fn parse_repo_add(args: &[String]) -> Result<RepoAdd, String> {
    let mut positional: Vec<String> = Vec::new();
    let mut key = None;
    let mut allow_unsigned = false;
    let mut architecture = "amd64".to_string();

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        i += 1;
        match arg {
            "--key" | "-k" => {
                key = Some(
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| "--key needs a file".to_string())?,
                );
                i += 1;
            }
            "--arch" => {
                architecture = args
                    .get(i)
                    .cloned()
                    .ok_or_else(|| "--arch needs a value".to_string())?;
                i += 1;
            }
            "--allow-unsigned" => allow_unsigned = true,
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => positional.push(other.to_string()),
        }
    }

    if positional.len() < 3 {
        return Err(
            "usage: senbit-pkg repo add <name> <uri> <suite> [components...] --key <file>\n\
             \x20      (--allow-unsigned to accept a repository without signature: NOT recommended)"
                .to_string(),
        );
    }
    let name = positional[0].clone();
    let uri = positional[1].clone();
    let suite = positional[2].clone();
    let components = positional[3..].to_vec();

    if !valid_name(&name) {
        return Err(format!("`{name}` is not a valid repository name (letters, digits, - _ .)"));
    }
    if !valid_uri(&uri) {
        return Err(format!("`{uri}` is not a valid URI (http://, https:// or file:/)"));
    }
    if key.is_none() && !allow_unsigned {
        return Err(
            "a signature key is required: --key <file.gpg|file.asc>\n\
             (the repository's packages are verified with it; --allow-unsigned disables the check)"
                .to_string(),
        );
    }
    Ok(RepoAdd { name, uri, suite, components, key, allow_unsigned, architecture })
}

fn repo_add(args: &[String]) -> Result<(), String> {
    let r = parse_repo_add(args)?;

    let signed_by = match &r.key {
        Some(key) => {
            let source = Path::new(key);
            if !source.is_file() {
                return Err(format!("key file not found: {key}"));
            }
            fs::create_dir_all(KEYRING_DIR).map_err(|e| format!("cannot create {KEYRING_DIR}: {e}"))?;
            let target = format!("{KEYRING_DIR}/{}.{}", r.name, key_extension(key));
            fs::copy(source, &target).map_err(|e| format!("cannot install the key: {e}"))?;
            println!("key installed: {target}");
            Some(target)
        }
        None => {
            eprintln!("warning: `{}` is added WITHOUT signature check", r.name);
            None
        }
    };

    let repo = Repo {
        uri: &r.uri,
        suite: &r.suite,
        components: &r.components,
        architecture: &r.architecture,
        signed_by: signed_by.as_deref(),
    };
    let text = render_sources(&r.name, &repo);
    let path = format!("{SOURCES_DIR}/{}.sources", r.name);
    fs::create_dir_all(SOURCES_DIR).map_err(|e| format!("cannot create {SOURCES_DIR}: {e}"))?;
    fs::write(&path, text).map_err(|e| format!("cannot write {path}: {e}"))?;
    println!("repository added: {path}");
    println!("now run: senbit-pkg update");
    let _ = r.allow_unsigned;
    Ok(())
}

fn repo_remove(args: &[String]) -> Result<(), String> {
    let name = args
        .first()
        .ok_or_else(|| "usage: senbit-pkg repo remove <name>".to_string())?;
    if !valid_name(name) {
        return Err(format!("`{name}` is not a valid repository name"));
    }
    let path = format!("{SOURCES_DIR}/{name}.sources");
    if !Path::new(&path).is_file() {
        return Err(format!("no such repository: {name}"));
    }
    fs::remove_file(&path).map_err(|e| format!("cannot remove {path}: {e}"))?;
    for ext in ["gpg", "asc"] {
        let _ = fs::remove_file(format!("{KEYRING_DIR}/{name}.{ext}"));
    }
    println!("repository removed: {name}");
    Ok(())
}

fn repo(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("list") => repo_list(),
        Some("add") => repo_add(&args[1..]),
        Some("remove") | Some("rm") => repo_remove(&args[1..]),
        Some(other) => Err(format!("unknown repo command `{other}` (list | add | remove)")),
    }
}

fn keys(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("list") => {
            let mut any = false;
            for dir in [KEYRING_DIR, "/usr/share/keyrings", "/etc/apt/trusted.gpg.d"] {
                let Ok(entries) = fs::read_dir(dir) else { continue };
                let mut names: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .filter(|n| n.ends_with(".gpg") || n.ends_with(".asc"))
                    .collect();
                names.sort();
                for name in names {
                    println!("{dir}/{name}");
                    any = true;
                }
            }
            if !any {
                println!("No key found.");
            }
            Ok(())
        }
        Some("add") => {
            let name = args.get(1).ok_or_else(|| "usage: senbit-pkg keys add <name> <file>".to_string())?;
            let file = args.get(2).ok_or_else(|| "usage: senbit-pkg keys add <name> <file>".to_string())?;
            if !valid_name(name) {
                return Err(format!("`{name}` is not a valid key name"));
            }
            if !Path::new(file).is_file() {
                return Err(format!("key file not found: {file}"));
            }
            fs::create_dir_all(KEYRING_DIR).map_err(|e| format!("cannot create {KEYRING_DIR}: {e}"))?;
            let target = format!("{KEYRING_DIR}/{name}.{}", key_extension(file));
            fs::copy(file, &target).map_err(|e| format!("cannot install the key: {e}"))?;
            println!("key installed: {target}");
            println!("use it with: senbit-pkg repo add <name> <uri> <suite> --key {target}");
            Ok(())
        }
        Some(other) => Err(format!("unknown keys command `{other}` (list | add)")),
    }
}

/* ------------------------------------------------------------------ */
/* Point d'entrée                                                     */
/* ------------------------------------------------------------------ */

fn usage() -> &'static str {
    "senbit-pkg - Senbit package manager (Debian .deb packages, apt/dpkg)\n\
     \n\
     packages:\n\
     \x20 senbit-pkg update                      refresh the package lists\n\
     \x20 senbit-pkg install <package...>        install (and start the services)\n\
     \x20 senbit-pkg remove|purge <package...>   remove (purge also deletes the configuration)\n\
     \x20 senbit-pkg upgrade | full-upgrade      update the installed packages\n\
     \x20 senbit-pkg autoremove | clean | fix    clean up, repair dependencies\n\
     \x20 senbit-pkg search <term> | show <package> | depends <package>\n\
     \x20 senbit-pkg list                        installed packages\n\
     \x20 senbit-pkg files <package>             files of a package\n\
     \x20 senbit-pkg owns <file>                 package that owns a file\n\
     \x20 senbit-pkg hold|unhold <package>       freeze / unfreeze a version\n\
     \n\
     repositories and signatures:\n\
     \x20 senbit-pkg repo list\n\
     \x20 senbit-pkg repo add <name> <uri> <suite> [components...] --key <file.gpg|file.asc>\n\
     \x20 senbit-pkg repo remove <name>\n\
     \x20 senbit-pkg keys list | add <name> <file>\n\
     \n\
     options after the command go to apt (e.g. senbit-pkg install -y curl).\n\
     --interactive keeps debconf questions (default: non-interactive)."
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    let interactive = args.iter().any(|a| a == "--interactive");
    args.retain(|a| a != "--interactive");

    let Some(command) = args.first().cloned() else {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    };
    let rest: Vec<String> = args[1..].to_vec();

    match command.as_str() {
        "help" | "-h" | "--help" => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        "version" | "--version" | "-V" => {
            println!("senbit-pkg {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    if needs_root(&command, &rest) && !is_root() {
        return reexec_with_sudo(&args);
    }

    match command.as_str() {
        "repo" => finish(repo(&rest)),
        "keys" => finish(keys(&rest)),
        "list" => {
            // Paquets installés : nom, version.
            let mut a = vec!["-W".to_string(), "-f=${Package}\t${Version}\n".to_string()];
            a.extend(rest);
            exec("dpkg-query", &a, interactive)
        }
        other => match apt_command(other) {
            Some(parts) => {
                let mut a: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
                a.extend(rest);
                exec(parts[0], &a, interactive)
            }
            None => {
                eprintln!("senbit-pkg: unknown command `{other}`\n\n{}", usage());
                ExitCode::FAILURE
            }
        },
    }
}

fn finish(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("senbit-pkg: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn names_and_uris_are_validated() {
        assert!(valid_name("docker"));
        assert!(valid_name("my-repo_1.2"));
        assert!(!valid_name(""));
        assert!(!valid_name("../etc"));
        assert!(!valid_name("a b"));
        assert!(!valid_name(".hidden"));
        assert!(valid_uri("https://download.docker.com/linux/debian"));
        assert!(!valid_uri("ftp://x"));
        assert!(!valid_uri("https://a b"));
    }

    #[test]
    fn signed_repository_file() {
        let components = vec!["stable".to_string(), "main".to_string()];
        let repo = Repo {
            uri: "https://example.org/debian",
            suite: "bookworm",
            components: &components,
            architecture: "amd64",
            signed_by: Some("/etc/apt/keyrings/example.gpg"),
        };
        let text = render_sources("example", &repo);
        assert!(text.contains("Types: deb\n"));
        assert!(text.contains("URIs: https://example.org/debian\n"));
        assert!(text.contains("Suites: bookworm\n"));
        assert!(text.contains("Components: stable main\n"));
        assert!(text.contains("Signed-By: /etc/apt/keyrings/example.gpg\n"));
        assert!(!text.contains("Trusted"));

        let (uris, suites) = summarize_sources(&text);
        assert_eq!(uris, "https://example.org/debian");
        assert_eq!(suites, "bookworm");
    }

    #[test]
    fn unsigned_repository_is_marked_trusted_and_defaults_to_main() {
        let repo = Repo {
            uri: "http://localhost/repo",
            suite: "stable",
            components: &[],
            architecture: "amd64",
            signed_by: None,
        };
        let text = render_sources("local", &repo);
        assert!(text.contains("Components: main\n"));
        assert!(text.contains("Trusted: yes\n"));
        assert!(!text.contains("Signed-By"));
    }

    #[test]
    fn repo_add_requires_a_key_unless_allowed() {
        assert!(parse_repo_add(&args(&["x", "https://e.org/d", "bookworm"])).is_err());

        let ok = parse_repo_add(&args(&["x", "https://e.org/d", "bookworm", "main", "contrib", "--key", "k.gpg"])).unwrap();
        assert_eq!(ok.components, vec!["main", "contrib"]);
        assert_eq!(ok.key.as_deref(), Some("k.gpg"));

        let unsigned = parse_repo_add(&args(&["x", "https://e.org/d", "bookworm", "--allow-unsigned"])).unwrap();
        assert!(unsigned.allow_unsigned && unsigned.key.is_none());

        assert!(parse_repo_add(&args(&["x", "https://e.org/d"])).is_err());
        assert!(parse_repo_add(&args(&["../x", "https://e.org/d", "s", "--key", "k"])).is_err());
        assert!(parse_repo_add(&args(&["x", "ftp://e.org/d", "s", "--key", "k"])).is_err());
        assert!(parse_repo_add(&args(&["x", "https://e.org/d", "s", "--bogus"])).is_err());
    }

    #[test]
    fn key_extensions() {
        assert_eq!(key_extension("docker.asc"), "asc");
        assert_eq!(key_extension("docker.gpg"), "gpg");
        assert_eq!(key_extension("docker"), "gpg");
    }

    #[test]
    fn commands_map_to_apt_and_dpkg() {
        assert_eq!(apt_command("install").unwrap(), vec!["apt-get", "install"]);
        assert_eq!(apt_command("search").unwrap(), vec!["apt-cache", "search"]);
        assert_eq!(apt_command("files").unwrap(), vec!["dpkg", "-L"]);
        assert!(apt_command("bogus").is_none());
    }

    #[test]
    fn root_is_needed_to_change_the_system_only() {
        assert!(needs_root("install", &[]));
        assert!(needs_root("update", &[]));
        assert!(!needs_root("search", &[]));
        assert!(!needs_root("list", &[]));
        assert!(needs_root("repo", &args(&["add"])));
        assert!(!needs_root("repo", &args(&["list"])));
        assert!(!needs_root("keys", &args(&["list"])));
    }
}
