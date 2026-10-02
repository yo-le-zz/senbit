use anyhow::{
    bail,
    Result,
};

use crate::system::handler::common::prepare_shutdown;

pub fn handle() -> Result<()> {
    prepare_shutdown()?;

    let result =
        unsafe {
            libc::reboot(
                libc::RB_POWER_OFF,
            )
        };

    if result != 0 {
        bail!(
            "shutdown failed: {}",
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}