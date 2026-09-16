//! Streaming content hashing.

use scene_core_protocol::Sha256Digest;
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Streams `path` through SHA-256 without loading it into memory.
pub fn hash_file(path: &Path) -> io::Result<Sha256Digest> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(Sha256Digest::new(format!("sha256:{hex}")).expect("digest is well-formed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_the_in_memory_reference() {
        let path = std::env::temp_dir().join(format!("scene-core-hash-{}", std::process::id()));
        std::fs::write(&path, b"scene-core").expect("write");
        assert_eq!(
            hash_file(&path).expect("hash"),
            Sha256Digest::from_bytes(b"scene-core")
        );
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod stack_tests {
    use super::*;

    #[test]
    fn hashing_fits_a_small_thread_stack() {
        let path =
            std::env::temp_dir().join(format!("scene-core-hash-stack-{}", std::process::id()));
        std::fs::write(&path, vec![7_u8; 3 * 1024 * 1024]).expect("write");
        let expected = Sha256Digest::from_bytes(&vec![7_u8; 3 * 1024 * 1024]);
        let worker = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let digest = hash_file(&path).expect("hash");
                (digest, path)
            })
            .expect("spawn");
        let (digest, path) = worker.join().expect("join");
        assert_eq!(digest, expected);
        let _ = std::fs::remove_file(&path);
    }
}
