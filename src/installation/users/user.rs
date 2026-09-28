// installation/users/user.rs

use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::installation::crypto::hash::hash_password;
use crate::installation::users::questions::ask_user;
use crate::logln;

pub struct User {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub password_hash: String,
}

impl User {
    pub fn new(
        name: String,
        uid: u32,
        gid: u32,
        home: String,
        shell: String,
        password: &str,
    ) -> Result<Self> {
        let password_hash = hash_password(password)?;

        Ok(Self {
            name,
            uid,
            gid,
            home,
            shell,
            password_hash,
        })
    }

    pub fn new_auto(
        root: &Path,
        name: String,
        home: String,
        shell: String,
        password: &str,
    ) -> Result<Self> {
        let uid = find_next_uid(root)?;
        let gid = find_next_gid(root)?;

        Self::new(
            name,
            uid,
            gid,
            home,
            shell,
            password,
        )
    }

    pub fn create(&self, root: &Path) -> Result<()> {
        let etc = root.join("etc");

        fs::create_dir_all(&etc)
            .context("failed to create /etc")?;

        let home_path = self
            .home
            .strip_prefix('/')
            .unwrap_or(&self.home);

        let home = root.join(home_path);

        fs::create_dir_all(&home)
            .with_context(|| {
                format!("failed to create home directory {:?}", home)
            })?;

        fs::set_permissions(
            &home,
            fs::Permissions::from_mode(0o700),
        )
        .context("failed to set home directory permissions")?;

        let passwd = etc.join("passwd");

        let passwd_entry = format!(
            "{}:x:{}:{}::{}:{}\n",
            self.name,
            self.uid,
            self.gid,
            self.home,
            self.shell
        );

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&passwd)
            .context("failed to open /etc/passwd")?
            .write_all(passwd_entry.as_bytes())
            .context("failed to write /etc/passwd")?;

        fs::set_permissions(
            &passwd,
            fs::Permissions::from_mode(0o644),
        )
        .context("failed to set /etc/passwd permissions")?;

        let shadow = etc.join("shadow");

        let shadow_entry = format!(
            "{}:{}:0:0:99999:7:::\n",
            self.name,
            self.password_hash
        );

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&shadow)
            .context("failed to open /etc/shadow")?
            .write_all(shadow_entry.as_bytes())
            .context("failed to write /etc/shadow")?;

        fs::set_permissions(
            &shadow,
            fs::Permissions::from_mode(0o600),
        )
        .context("failed to set /etc/shadow permissions")?;

        let group = etc.join("group");

        let group_entry = format!(
            "{}:x:{}:\n",
            self.name,
            self.gid
        );

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&group)
            .context("failed to open /etc/group")?
            .write_all(group_entry.as_bytes())
            .context("failed to write /etc/group")?;

        fs::set_permissions(
            &group,
            fs::Permissions::from_mode(0o644),
        )
        .context("failed to set /etc/group permissions")?;

        Ok(())
    }
}

fn find_next_uid(root: &Path) -> Result<u32> {
    let passwd = root.join("etc/passwd");

    if !passwd.exists() {
        return Ok(1000);
    }

    let content = fs::read_to_string(&passwd)
        .context("failed to read /etc/passwd")?;

    let mut next_uid = 1000;

    for line in content.lines() {
        let Some(uid) = line.split(':').nth(2) else {
            continue;
        };

        let Ok(uid) = uid.parse::<u32>() else {
            continue;
        };

        if uid >= next_uid {
            next_uid = uid + 1;
        }
    }

    Ok(next_uid)
}

fn find_next_gid(root: &Path) -> Result<u32> {
    let group = root.join("etc/group");

    if !group.exists() {
        return Ok(1000);
    }

    let content = fs::read_to_string(&group)
        .context("failed to read /etc/group")?;

    let mut next_gid = 1000;

    for line in content.lines() {
        let Some(gid) = line.split(':').nth(2) else {
            continue;
        };

        let Ok(gid) = gid.parse::<u32>() else {
            continue;
        };

        if gid >= next_gid {
            next_gid = gid + 1;
        }
    }

    Ok(next_gid)
}

pub fn setup_users(mount_point: &Path) -> Result<(), String> {
    logln!("Creating root account...");

    let root_user = User::new(
        "root".to_string(),
        0,
        0,
        "/root".to_string(),
        "/bin/sh".to_string(),
        "*",
    )
    .map_err(|e| format!("Failed to create root user: {}", e))?;

    root_user
        .create(mount_point)
        .map_err(|e| format!("Failed to create root account: {}", e))?;

    logln!("Root account created.");

    logln!("Creating user account...");

    let user = ask_user(mount_point)
        .map_err(|e| format!("User configuration failed: {}", e))?;

    user.create(mount_point)
        .map_err(|e| format!("Failed to create user account: {}", e))?;

    logln!(
        "User '{}' created successfully (UID {}, GID {}).",
        user.name,
        user.uid,
        user.gid
    );

    Ok(())
}