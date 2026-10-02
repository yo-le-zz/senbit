use anyhow::{
    bail,
    Result,
};

use crate::system::handler::common::prepare_shutdown;

pub fn handle() -> Result<()> {
    /*
     * ========================================================
     * Prepare system shutdown
     * ========================================================
     */

    prepare_shutdown()?;

    /*
     * ========================================================
     * Final poweroff
     * ========================================================
     */

    let result =
        unsafe {
            libc::reboot(
                libc::RB_POWER_OFF,
            )
        };

    if result != 0 {
        bail!(
            "poweroff failed: {}",
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}