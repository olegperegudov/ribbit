//! Files this app creates are for its owner only.
//!
//! Everything Ribbit writes — the API keys in `.env`, the transcript log, the
//! vocabulary, the session log — is either a secret or a record of what the user
//! said out loud. `std::fs::write` obeys the
//! umask, which on a default macOS account is 022: the file lands world-readable
//! and any process running as any user on the machine can read the lot. So the
//! writes go through here instead, and the mode is set at creation rather
//! than after: a `write` followed by a `set_permissions` leaves a window in
//! which the secret is already on disk and still readable by everyone.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

/// The directory and everything below it belongs to the owner (0700 on unix).
/// On Windows the per-user AppData root is already ACL'd to the user.
pub fn create_dir(path: &Path) -> std::io::Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    b.mode(0o700);
    b.create(path)
}

/// Create-or-truncate `path` and write `bytes` into it with mode 0600.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let mut f = opts.open(path)?;
    // `mode` above only applies to a file we just created. A history file an
    // older build left world-readable would keep its old mode, so narrow the
    // open handle too — still before any content goes in.
    narrow(&f)?;
    f.write_all(bytes)
}

/// Open `path` for appending, creating it with mode 0600 if it is not there.
pub fn append(path: &Path) -> std::io::Result<std::fs::File> {
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    opts.mode(0o600);
    let f = opts.open(path)?;
    narrow(&f)?;
    Ok(f)
}

#[cfg(unix)]
fn narrow(f: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    f.set_permissions(std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn narrow(_f: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_written_file_is_readable_by_its_owner_and_nobody_else() {
        let dir = std::env::temp_dir().join(format!("ribbit-private-{}", std::process::id()));
        create_dir(&dir).unwrap();
        let f = dir.join("secret");
        write(&f, b"password").unwrap();
        assert_eq!(mode(&f), 0o600, "an API key must not be world-readable");
        assert_eq!(mode(&dir), 0o700, "the folder must not be world-listable");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rewriting_a_file_does_not_widen_it() {
        let dir = std::env::temp_dir().join(format!("ribbit-private-rw-{}", std::process::id()));
        create_dir(&dir).unwrap();
        let f = dir.join(".env");
        // A key file left world-readable by an older build must come back locked.
        std::fs::write(&f, b"old").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        write(&f, b"new").unwrap();
        assert_eq!(mode(&f), 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
