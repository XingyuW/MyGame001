//! Opened-handle, regular-file bounded input for simulation data, maps and client settings.
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug)]
pub enum BoundedFileError {
    Read(io::Error),
    TooLarge { max_bytes: u64 },
}
impl From<io::Error> for BoundedFileError {
    fn from(error: io::Error) -> Self {
        Self::Read(error)
    }
}

fn read_opened_regular(file: File, max_bytes: u64) -> Result<Vec<u8>, BoundedFileError> {
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file").into());
    }
    let mut bytes = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > max_bytes) {
        return Err(BoundedFileError::TooLarge { max_bytes });
    }
    Ok(bytes)
}

/// Follow a regular-file symlink, but never block opening a Unix FIFO; validate the opened
/// handle rather than trusting metadata obtained from the pathname before opening it.
pub fn read_bounded_regular(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BoundedFileError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    read_opened_regular(options.open(path)?, max_bytes)
}

// Append as #[cfg(test)] mod tests inside new sim::fileguard module.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ra9_opened_handle_survives_path_replacement_and_has_exact_bounds() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = (0..32)
            .find_map(|_| {
                let candidate = std::env::temp_dir().join(format!(
                    "mygame_ra9_handle_{}_{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match std::fs::create_dir(&candidate) {
                    Ok(()) => Some(candidate),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => panic!("exclusive tiny fixture directory: {error}"),
                }
            })
            .expect("fresh per-invocation fixture directory");
        let path = dir.join("input");
        std::fs::write(&path, b"abc").expect("write");
        assert_eq!(read_bounded_regular(&path, 3).expect("exact"), b"abc");
        assert!(matches!(
            read_bounded_regular(&path, 2),
            Err(BoundedFileError::TooLarge { max_bytes: 2 })
        ));
        let file = File::open(&path).expect("opened regular handle");
        std::fs::remove_file(&path).expect("unlink own path");
        #[cfg(unix)]
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .expect("mkfifo")
                .success()
        );
        #[cfg(not(unix))]
        std::fs::create_dir(&path).expect("replacement nonregular");
        assert_eq!(
            read_opened_regular(file, 3).expect("same checked handle"),
            b"abc"
        );
        #[cfg(unix)]
        std::fs::remove_file(&path).expect("own FIFO");
        #[cfg(not(unix))]
        std::fs::remove_dir(&path).expect("own directory");
        std::fs::write(&path, b"").expect("empty file");
        assert_eq!(
            read_bounded_regular(&path, 0).expect("zero bound empty"),
            b""
        );
        std::fs::write(&path, b"a").expect("one byte");
        assert!(matches!(
            read_bounded_regular(&path, 0),
            Err(BoundedFileError::TooLarge { max_bytes: 0 })
        ));
        std::fs::remove_dir_all(dir).expect("own tiny fixtures");
    }
    #[cfg(unix)]
    fn ra9_owned_child(test: &str, mode: &str, path: &std::path::Path) {
        struct Owned(std::process::Child);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Owned(
            std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args(["--exact", test, "--nocapture"])
                .env("MYGAME_RA9_BOUNDARY_CHILD_MODE", mode)
                .env("MYGAME_RA9_BOUNDARY_CHILD_PATH", path)
                .spawn()
                .expect("owned bounded child"),
        );
        let started = std::time::Instant::now();
        loop {
            if let Some(status) = child.0.try_wait().expect("child status") {
                assert!(status.success(), "native child test failed: {status}");
                return;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "native child blocked; owned child is killed and waited during unwinding"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    #[cfg(unix)]
    fn ra9_native_data_loader_fifo_read_is_owned_and_contextual() {
        if std::env::var_os("MYGAME_RA9_BOUNDARY_CHILD_MODE").is_some() {
            let path = std::path::PathBuf::from(
                std::env::var_os("MYGAME_RA9_BOUNDARY_CHILD_PATH").expect("path"),
            );
            let economy = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/data/economy.ron");
            let error = crate::load_definitions(&path, economy)
                .expect_err("native data loader rejects FIFO");
            assert_eq!(error.file, path);
            assert_eq!(error.field, "file");
            assert_eq!(
                error.kind,
                crate::DataErrorKind::Read("not a regular file".into())
            );
            return;
        }
        let path =
            std::env::temp_dir().join(format!("mygame_ra9_data_fifo_{}", std::process::id()));
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .expect("mkfifo")
                .success()
        );
        ra9_owned_child(
            "fileguard::tests::ra9_native_data_loader_fifo_read_is_owned_and_contextual",
            "data",
            &path,
        );
        std::fs::remove_file(path).expect("own tiny FIFO");
    }
}
