//! Lecture / écriture de /etc/passwd, /etc/group, /etc/shadow et /etc/gshadow.
//!
//! Partagé par `adduser` (adduser, addgroup, deluser, delgroup) et `useradd`
//! (useradd, groupadd, userdel, groupdel, usermod) : les scripts de mainteneur
//! des paquets Debian appellent ces commandes (postgresql, nginx, redis...).
//!
//! Ce dossier n'a pas de Cargo.toml : `devtool` ne le prend pas pour une
//! commande ; chaque commande l'inclut avec
//! `#[path = "../../_shared/accounts.rs"] mod accounts;`.
//!
//! Le dossier des fichiers est /etc, ou $SENBIT_ETC (tests).
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SYSTEM_ID_MIN: u32 = 100;
pub const SYSTEM_ID_MAX: u32 = 999;
pub const USER_ID_MIN: u32 = 1000;
pub const USER_ID_MAX: u32 = 59999;

pub fn etc_dir() -> PathBuf {
    std::env::var_os("SENBIT_ETC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc"))
}

/// Jours depuis le 1er janvier 1970 (champ « dernière modification » de shadow).
pub fn today_days() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

/// Nom d'utilisateur / de groupe acceptable : ni vide, ni plus de 32
/// caractères, pas de `:` `,` `/` ni d'espace, ne commence pas par `-`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '$' | '@'))
}

/* ------------------------------------------------------------------ */
/* Un fichier de la base                                              */
/* ------------------------------------------------------------------ */

/// Un fichier ligne par ligne (`nom:champ:champ...`). Les lignes que l'on ne
/// comprend pas (commentaires, entrées NIS `+`) sont conservées telles quelles.
pub struct Db {
    pub path: PathBuf,
    pub lines: Vec<String>,
    mode: u32,
}

fn first_field(line: &str) -> &str {
    line.split(':').next().unwrap_or("")
}

impl Db {
    pub fn load(path: PathBuf, mode: u32) -> io::Result<Db> {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e),
        };
        Ok(Db {
            path,
            lines: text.lines().map(|l| l.to_string()).collect(),
            mode,
        })
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.lines.iter().position(|l| first_field(l) == name)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.find(name) {
            Some(i) => {
                self.lines.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn push(&mut self, line: String) {
        self.lines.push(line);
    }

    /// Écriture atomique : fichier temporaire dans le même dossier, puis rename.
    pub fn save(&self) -> io::Result<()> {
        let mut text = self.lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        let tmp = self.path.with_extension("senbit-tmp");
        fs::write(&tmp, text)?;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(self.mode))?;
        fs::rename(&tmp, &self.path)
    }
}

/* ------------------------------------------------------------------ */
/* Lignes typées                                                      */
/* ------------------------------------------------------------------ */

#[derive(Debug, Clone)]
pub struct User {
    pub name: String,
    pub passwd: String,
    pub uid: u32,
    pub gid: u32,
    pub gecos: String,
    pub home: String,
    pub shell: String,
}

impl User {
    pub fn parse(line: &str) -> Option<User> {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 || f[0].is_empty() {
            return None;
        }
        Some(User {
            name: f[0].to_string(),
            passwd: f[1].to_string(),
            uid: f[2].parse().ok()?,
            gid: f[3].parse().ok()?,
            gecos: f[4].to_string(),
            home: f[5].to_string(),
            shell: f[6].to_string(),
        })
    }

    pub fn line(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            self.name, self.passwd, self.uid, self.gid, self.gecos, self.home, self.shell
        )
    }
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub passwd: String,
    pub gid: u32,
    pub members: Vec<String>,
}

fn split_members(field: &str) -> Vec<String> {
    field
        .split(',')
        .filter(|m| !m.is_empty())
        .map(|m| m.to_string())
        .collect()
}

impl Group {
    pub fn parse(line: &str) -> Option<Group> {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 3 || f[0].is_empty() {
            return None;
        }
        Some(Group {
            name: f[0].to_string(),
            passwd: f[1].to_string(),
            gid: f[2].parse().ok()?,
            members: f.get(3).map(|m| split_members(m)).unwrap_or_default(),
        })
    }

    pub fn line(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.name,
            self.passwd,
            self.gid,
            self.members.join(",")
        )
    }
}

/// Remplace le 4e champ (membres) d'une ligne `group` ou `gshadow`.
fn with_members(line: &str, members: &[String]) -> String {
    let mut fields: Vec<String> = line.split(':').map(|f| f.to_string()).collect();
    while fields.len() < 4 {
        fields.push(String::new());
    }
    fields[3] = members.join(",");
    fields.join(":")
}

fn members_of(line: &str) -> Vec<String> {
    line.split(':').nth(3).map(split_members).unwrap_or_default()
}

/* ------------------------------------------------------------------ */
/* Les quatre fichiers ensemble                                       */
/* ------------------------------------------------------------------ */

pub struct Accounts {
    pub passwd: Db,
    pub group: Db,
    pub shadow: Db,
    pub gshadow: Db,
}

impl Accounts {
    pub fn load() -> io::Result<Accounts> {
        let etc = etc_dir();
        Ok(Accounts {
            passwd: Db::load(etc.join("passwd"), 0o644)?,
            group: Db::load(etc.join("group"), 0o644)?,
            shadow: Db::load(etc.join("shadow"), 0o600)?,
            gshadow: Db::load(etc.join("gshadow"), 0o600)?,
        })
    }

    pub fn save(&self) -> io::Result<()> {
        self.passwd.save()?;
        self.group.save()?;
        self.shadow.save()?;
        self.gshadow.save()
    }

    pub fn users(&self) -> Vec<User> {
        self.passwd
            .lines
            .iter()
            .filter_map(|l| User::parse(l))
            .collect()
    }

    pub fn groups(&self) -> Vec<Group> {
        self.group
            .lines
            .iter()
            .filter_map(|l| Group::parse(l))
            .collect()
    }

    pub fn user(&self, name: &str) -> Option<User> {
        self.users().into_iter().find(|u| u.name == name)
    }

    pub fn user_by_uid(&self, uid: u32) -> Option<User> {
        self.users().into_iter().find(|u| u.uid == uid)
    }

    pub fn group_named(&self, name: &str) -> Option<Group> {
        self.groups().into_iter().find(|g| g.name == name)
    }

    pub fn group_by_gid(&self, gid: u32) -> Option<Group> {
        self.groups().into_iter().find(|g| g.gid == gid)
    }

    /// Plus petit UID libre de la plage système (100-999) ou utilisateur (1000+).
    pub fn free_uid(&self, system: bool) -> Option<u32> {
        let used: BTreeSet<u32> = self.users().iter().map(|u| u.uid).collect();
        first_free(&used, system)
    }

    pub fn free_gid(&self, system: bool) -> Option<u32> {
        let used: BTreeSet<u32> = self.groups().iter().map(|g| g.gid).collect();
        first_free(&used, system)
    }

    pub fn gid_in_use(&self, gid: u32) -> bool {
        self.group_by_gid(gid).is_some()
    }

    pub fn uid_in_use(&self, uid: u32) -> bool {
        self.user_by_uid(uid).is_some()
    }

    pub fn add_group(&mut self, name: &str, gid: u32, members: &[String]) {
        let group = Group {
            name: name.to_string(),
            passwd: "x".to_string(),
            gid,
            members: members.to_vec(),
        };
        self.group.push(group.line());
        self.gshadow
            .push(format!("{}:!::{}", name, members.join(",")));
    }

    /// `hash` : "!" (verrouillé), "*" (compte système) ou un hash de mot de passe.
    pub fn add_user(&mut self, user: &User, hash: &str) {
        let mut entry = user.clone();
        entry.passwd = "x".to_string();
        self.passwd.push(entry.line());
        self.shadow.push(format!(
            "{}:{}:{}:0:99999:7:::",
            user.name,
            hash,
            today_days()
        ));
    }

    pub fn set_user(&mut self, user: &User) {
        if let Some(i) = self.passwd.find(&user.name) {
            self.passwd.lines[i] = user.line();
        }
    }

    /// Ajoute `user` aux membres de `group`. Faux si déjà membre ou groupe inconnu.
    pub fn add_member(&mut self, user: &str, group: &str) -> bool {
        let Some(i) = self.group.find(group) else {
            return false;
        };
        let mut members = members_of(&self.group.lines[i]);
        if members.iter().any(|m| m == user) {
            return false;
        }
        members.push(user.to_string());
        self.group.lines[i] = with_members(&self.group.lines[i], &members);
        if let Some(j) = self.gshadow.find(group) {
            self.gshadow.lines[j] = with_members(&self.gshadow.lines[j], &members);
        }
        true
    }

    /// Retire `user` des membres de `group`. Faux s'il n'en faisait pas partie.
    pub fn remove_member(&mut self, user: &str, group: &str) -> bool {
        let Some(i) = self.group.find(group) else {
            return false;
        };
        let mut members = members_of(&self.group.lines[i]);
        let before = members.len();
        members.retain(|m| m != user);
        if members.len() == before {
            return false;
        }
        self.group.lines[i] = with_members(&self.group.lines[i], &members);
        if let Some(j) = self.gshadow.find(group) {
            self.gshadow.lines[j] = with_members(&self.gshadow.lines[j], &members);
        }
        true
    }

    /// Retire `user` de TOUS les groupes.
    pub fn remove_from_all_groups(&mut self, user: &str) {
        let names: Vec<String> = self.groups().iter().map(|g| g.name.clone()).collect();
        for name in names {
            self.remove_member(user, &name);
        }
    }

    pub fn remove_user(&mut self, name: &str) -> bool {
        self.shadow.remove(name);
        self.passwd.remove(name)
    }

    pub fn remove_group(&mut self, name: &str) -> bool {
        self.gshadow.remove(name);
        self.group.remove(name)
    }
}

fn first_free(used: &BTreeSet<u32>, system: bool) -> Option<u32> {
    let (min, max) = if system {
        (SYSTEM_ID_MIN, SYSTEM_ID_MAX)
    } else {
        (USER_ID_MIN, USER_ID_MAX)
    };
    (min..=max).find(|id| !used.contains(id))
}

/* ------------------------------------------------------------------ */
/* Dossier personnel                                                  */
/* ------------------------------------------------------------------ */

/// Copie récursive de `from` vers `to` (liens conservés), tout appartient à uid:gid.
pub fn copy_tree_owned(from: &Path, to: &Path, uid: u32, gid: u32) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let kind = fs::symlink_metadata(&src)?.file_type();
        if kind.is_symlink() {
            let target = fs::read_link(&src)?;
            let _ = fs::remove_file(&dst);
            std::os::unix::fs::symlink(target, &dst)?;
        } else if kind.is_dir() {
            copy_tree_owned(&src, &dst, uid, gid)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst)?;
        } else {
            continue;
        }
        if !kind.is_symlink() {
            chown(&dst, Some(uid), Some(gid))?;
        }
    }
    chown(to, Some(uid), Some(gid))?;
    Ok(())
}

/// Crée le dossier personnel (mode donné), copie `skel` si demandé.
pub fn create_home(
    home: &str,
    uid: u32,
    gid: u32,
    mode: u32,
    skel: Option<&Path>,
) -> io::Result<()> {
    let path = Path::new(home);
    fs::create_dir_all(path)?;
    if let Some(skel) = skel {
        if skel.is_dir() {
            copy_tree_owned(skel, path, uid, gid)?;
        }
    }
    chown(path, Some(uid), Some(gid))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

/// Supprime un dossier personnel, jamais un chemin dangereux.
pub fn remove_home(home: &str) -> io::Result<()> {
    let path = Path::new(home);
    let dangerous = ["", "/", "/nonexistent", "/root", "/home", "/usr", "/var", "/etc", "/bin"];
    if dangerous.contains(&home) || !path.is_absolute() {
        return Ok(());
    }
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub fn is_root() -> bool {
    // /proc/self appartient à l'UID effectif du processus.
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/proc/self").map(|m| m.uid() == 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account_fixture() -> Accounts {
        let db = |lines: &[&str], mode| Db {
            path: PathBuf::from("/nonexistent/unused"),
            lines: lines.iter().map(|l| l.to_string()).collect(),
            mode,
        };
        Accounts {
            passwd: db(&["root:x:0:0:root:/root:/bin/sh"], 0o644),
            group: db(&["root:x:0:", "staff:x:50:alice"], 0o644),
            shadow: db(&["root:!:1:0:99999:7:::"], 0o600),
            gshadow: db(&["root:!::", "staff:!::alice"], 0o600),
        }
    }

    #[test]
    fn name_validation() {
        assert!(valid_name("postgres"));
        assert!(valid_name("_apt"));
        assert!(valid_name("www-data"));
        assert!(!valid_name(""));
        assert!(!valid_name("a:b"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("-x"));
        assert!(!valid_name(&"a".repeat(33)));
    }

    #[test]
    fn free_ids_follow_the_ranges() {
        let acc = account_fixture();
        assert_eq!(acc.free_uid(true), Some(100));
        assert_eq!(acc.free_uid(false), Some(1000));
        assert_eq!(acc.free_gid(true), Some(100));
    }

    #[test]
    fn add_and_remove_user_and_group() {
        let mut acc = account_fixture();
        acc.add_group("postgres", 100, &[]);
        let user = User {
            name: "postgres".into(),
            passwd: "x".into(),
            uid: 100,
            gid: 100,
            gecos: "PostgreSQL".into(),
            home: "/var/lib/postgresql".into(),
            shell: "/bin/bash".into(),
        };
        acc.add_user(&user, "*");
        assert_eq!(acc.user("postgres").unwrap().home, "/var/lib/postgresql");
        assert!(acc.shadow.find("postgres").is_some());
        assert!(acc.gshadow.find("postgres").is_some());
        assert_eq!(acc.free_uid(true), Some(101));

        assert!(acc.add_member("postgres", "staff"));
        assert!(!acc.add_member("postgres", "staff"));
        assert!(!acc.add_member("postgres", "nope"));
        assert_eq!(
            acc.group_named("staff").unwrap().members,
            vec!["alice", "postgres"]
        );
        assert!(acc.gshadow.lines[1].ends_with("alice,postgres"));

        acc.remove_from_all_groups("postgres");
        assert_eq!(acc.group_named("staff").unwrap().members, vec!["alice"]);

        assert!(acc.remove_user("postgres"));
        assert!(acc.shadow.find("postgres").is_none());
        assert!(acc.remove_group("postgres"));
        assert!(acc.group_named("postgres").is_none());
    }

    #[test]
    fn unparsable_lines_are_kept() {
        let mut acc = account_fixture();
        acc.passwd.lines.push("+::::::".to_string());
        assert_eq!(acc.users().len(), 1);
        assert_eq!(acc.passwd.lines.len(), 2);
    }

    #[test]
    fn save_is_atomic_and_keeps_the_mode() {
        let dir = std::env::temp_dir().join(format!("senbit-accounts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let db = Db {
            path: dir.join("shadow"),
            lines: vec!["root:!:1:0:99999:7:::".to_string()],
            mode: 0o600,
        };
        db.save().unwrap();
        let meta = fs::metadata(dir.join("shadow")).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert!(!dir.join("shadow.senbit-tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
