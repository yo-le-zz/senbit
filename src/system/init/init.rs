use anyhow::Result;

use std::env::set_var;

use crate::filesystem::fs::mount;
use crate::system::init::hostname::get_hostname;

use std::os::unix::fs::symlink;

pub fn mount_system(
    sys_disk: &str,
    root: &str,
) -> Result<()> {
    let device =
        if sys_disk.starts_with("/dev/") {
            sys_disk.to_string()
        } else {
            format!("/dev/{}", sys_disk)
        };

    mount(
        &device,
        root,
        Some("ext4"),
    )?;

    mount(
        "proc",
        &format!("{}/proc", root),
        Some("proc"),
    )?;

    mount(
        "sysfs",
        &format!("{}/sys", root),
        Some("sysfs"),
    )?;

    mount(
        "devtmpfs",
        &format!("{}/dev", root),
        Some("devtmpfs"),
    )?;

    mount(
        "tmpfs",
        &format!("{}/run", root),
        Some("tmpfs"),
    )?;

    let var_dir =
        format!("{}/var", root);

    let var_run =
        format!("{}/var/run", root);

    std::fs::create_dir_all(
        &var_dir
    )?;

    if !std::path::Path::new(
        &var_run
    ).exists() {
        symlink(
            "/run",
            &var_run
        )?;
    }

    Ok(())
}

pub fn fstab_is_ok(root: &str) -> bool {
    std::path::Path::new(root)
        .join("etc/fstab")
        .exists()
}

pub fn init_vars(root: &str) -> Result<()> {
    let hostname = get_hostname(root)?;

    unsafe {
        set_var("HOSTNAME", hostname);
    }

    Ok(())
}