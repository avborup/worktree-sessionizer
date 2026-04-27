use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

pub fn cow_clone(src: &Path, dst: &Path) -> Result<()> {
    if cfg!(target_os = "macos") {
        if clonefile(src, dst).is_ok() {
            return Ok(());
        }
        let status = Command::new("/bin/cp")
            .args(["-Rc"])
            .arg(src)
            .arg(dst)
            .status()
            .context("cp -Rc")?;
        if !status.success() {
            anyhow::bail!("cp -Rc failed for {}", src.display());
        }
    } else {
        let status = Command::new("/bin/cp")
            .args(["-R", "--reflink=auto"])
            .arg(src)
            .arg(dst)
            .status()
            .context("cp --reflink=auto")?;
        if !status.success() {
            anyhow::bail!("cp --reflink=auto failed for {}", src.display());
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn clonefile(src: &Path, dst: &Path) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let src_c = CString::new(src.as_os_str().as_bytes())?;
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;

    let ret = unsafe { libc_clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0) };
    if ret != 0 {
        anyhow::bail!("clonefile failed");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    #[link_name = "clonefile"]
    fn libc_clonefile(
        src: *const std::ffi::c_char,
        dst: *const std::ffi::c_char,
        flags: u32,
    ) -> std::ffi::c_int;
}

#[cfg(not(target_os = "macos"))]
fn clonefile(_src: &Path, _dst: &Path) -> Result<()> {
    anyhow::bail!("clonefile not available")
}
