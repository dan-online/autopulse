use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

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
