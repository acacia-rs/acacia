//! Pinned downloads: a file is fetched once and refused unless it has the expected SHA-1.

use std::io::Read;
use std::path::Path;

use sha1::{Digest, Sha1};

pub type Error = Box<dyn std::error::Error>;

pub fn get(url: &str) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    ureq::get(url).call()?.into_body().into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn sha1_of(bytes: &[u8]) -> String {
    format!("{:x}", Sha1::digest(bytes))
}

/// The file at `path` when it has `sha1`; otherwise what `fetch` returns, checked and written there.
pub fn pinned(path: &Path, sha1: &str, fetch: impl FnOnce() -> Result<Vec<u8>, Error>) -> Result<Vec<u8>, Error> {
    if let Some(bytes) = std::fs::read(path).ok().filter(|b| sha1_of(b) == sha1) {
        return Ok(bytes);
    }
    let bytes = fetch()?;
    let sum = sha1_of(&bytes);
    if sum != sha1 {
        return Err(format!("{}: downloaded {sum}, pinned {sha1}", path.display()).into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, &bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";

    #[test]
    fn a_pinned_file_is_fetched_once_and_a_wrong_one_refused() {
        let path = std::env::temp_dir().join(format!("lookbake-pinned-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        assert!(pinned(&path, ABC, || Ok(b"abd".to_vec())).is_err());
        assert!(!path.exists(), "a refused download is not kept");
        assert_eq!(pinned(&path, ABC, || Ok(b"abc".to_vec())).unwrap(), b"abc");
        assert_eq!(pinned(&path, ABC, || Err("not fetched again".into())).unwrap(), b"abc");
        std::fs::remove_file(&path).unwrap();
    }
}
