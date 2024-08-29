use std::{fs::File, path::PathBuf};

use sha2::{Digest, Sha256};

pub fn sha256checksum(file_path: &PathBuf) -> String {
    let mut file = File::open(file_path).unwrap();
    let mut hasher = Sha256::new();

    std::io::copy(&mut file, &mut hasher).unwrap();

    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_sha256checksum() {
        let file_path = PathBuf::from("/tmp/test_checksum.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hi!").unwrap();

        let checksum = sha256checksum(&file_path);

        std::fs::remove_file(&file_path).unwrap();

        assert_eq!(
            checksum,
            "c0ddd62c7717180e7ffb8a15bb9674d3ec92592e0b7ac7d1d5289836b4553be2"
        );
    }
}
