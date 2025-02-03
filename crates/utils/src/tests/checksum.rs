#[cfg(test)]
mod tests {
    use crate::checksum::sha256checksum;
    use std::{
        env,
        fs::{remove_file, File},
        io::Write,
    };

    #[test]
    fn test_sha256checksum() {
        let tmp_dir = env::temp_dir();
        let path = tmp_dir.join("test_checksum.txt");

        let mut file = File::create(&path).unwrap();
        file.write_all(b"hi!").unwrap();

        let checksum = sha256checksum(&path).unwrap();

        remove_file(&path).unwrap();

        assert_eq!(
            checksum,
            "c0ddd62c7717180e7ffb8a15bb9674d3ec92592e0b7ac7d1d5289836b4553be2"
        );
    }
}
