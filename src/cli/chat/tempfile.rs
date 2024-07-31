use rand::{rngs::StdRng, RngCore, SeedableRng};
use std::fmt::Write;
use std::fs::File;
use std::io::{self, ErrorKind};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    env::temp_dir,
    fs::OpenOptions,
    path::{Path, PathBuf},
};

/// Creates a temporary file in the directory specified by `temp_dir`. The filename
/// of the temporary file is generated as `base || hex || extension`, where `hex` is
/// a randomly generated string. The resulting file is only readable by the current user.
/// The function returns `ErrorKind::AlreadyExists` only after several retries.
pub(crate) fn create_temp_file(
    temp_dir: &PathBuf,
    base: &str,
    extention: &str,
) -> std::io::Result<(PathBuf, File)> {
    const RETRIES: usize = 16;

    let mut rng = StdRng::from_entropy();

    for _ in 0..RETRIES {
        let mut suffix = [0u8; 32];

        rng.fill_bytes(&mut suffix);

        let filename = {
            let mut f = String::new();

            f.push_str(base);

            for b in suffix {
                write!(f, "{:02x}", b).unwrap();
            }

            f.push_str(extention);

            f
        };

        let path = temp_dir.join(filename);

        let open_result = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path);

        match open_result {
            Ok(file) => return Ok((path, file)),
            Err(err) => {
                if matches!(err.kind(), ErrorKind::AlreadyExists) {
                    continue;
                }

                return Err(err);
            }
        }
    }

    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "failed to create a secure tempfile after {} retries",
            RETRIES
        ),
    ))
}

/// A temporary file which is automatically unlinked when dropped
pub(crate) struct Tempfile {
    path: PathBuf,
    file: File,
}

impl Tempfile {
    pub(crate) fn with_base_and_ext(base: &str, extention: &str) -> std::io::Result<Tempfile> {
        let temp_dir = temp_dir();

        Self::new(&temp_dir, base, extention)
    }

    pub(crate) fn new(
        temp_dir: &PathBuf,
        base: &str,
        extention: &str,
    ) -> std::io::Result<Tempfile> {
        let (path, file) = create_temp_file(temp_dir, base, extention)?;

        Ok(Tempfile { path, file })
    }

    pub(crate) fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }

    pub(crate) fn path_buf(&self) -> &PathBuf {
        &self.path
    }

    pub(crate) fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for Tempfile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).expect("failed to remove tempfile")
    }
}
