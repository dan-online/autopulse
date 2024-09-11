use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use sha2::{Digest, Sha256};

pub fn sha256checksum(file_path: &PathBuf) -> anyhow::Result<String> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::{fs::remove_file, io::Write};

    use super::*;

    #[test]
    fn test_sha256checksum() {
        let file_path = PathBuf::from("/tmp/test_checksum.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"hi!").unwrap();

        let checksum = sha256checksum(&file_path).unwrap();

        remove_file(&file_path).unwrap();

        assert_eq!(
            checksum,
            "c0ddd62c7717180e7ffb8a15bb9674d3ec92592e0b7ac7d1d5289836b4553be2"
        );
    }
}
