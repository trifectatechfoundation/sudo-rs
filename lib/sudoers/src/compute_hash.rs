pub fn sha2(bits: u16, path: &std::path::Path) -> Box<[u8]> {
    use digest::{Digest, DynDigest};

    fn compute_hash(
        mut hasher: Box<dyn DynDigest>,
        path: &std::path::Path,
    ) -> Result<Box<[u8]>, std::io::Error> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};
        let mut source = BufReader::new(File::open(path)?);

        while {
            let buffer = source.fill_buf()?;
            let size = buffer.len();
            hasher.update(buffer);
            source.consume(size);

            size != 0
        } {}

        Ok(hasher.finalize())
    }

    compute_hash(
        match bits {
            224 => Box::new(sha2::Sha224::new()),
            256 => Box::new(sha2::Sha256::new()),
            384 => Box::new(sha2::Sha384::new()),
            512 => Box::new(sha2::Sha512::new()),
            _ => panic!("SHA{bits} is not a valid hash"),
        },
        path,
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod test {
    #[test]
    fn check_hash() {
        let file_to_check = std::path::Path::new("/bin/more");
        use crate::basic_parser::parse_eval;
        use crate::tokens::Sha2;
        use std::process::Command;
        for bits in [224, 256, 384, 512] {
            let Sha2(valid) = parse_eval(
                (&std::str::from_utf8(
                    &Command::new(format!("/bin/sha{bits}sum"))
                        .arg(file_to_check)
                        .output()
                        .unwrap()
                        .stdout,
                )
                .unwrap())
                    .split_whitespace()
                    .collect::<Vec<_>>()[0],
            );
            assert_eq!(super::sha2(bits, file_to_check), valid);
        }
    }
}
