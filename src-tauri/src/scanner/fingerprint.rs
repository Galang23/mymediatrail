use blake3::Hasher;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const THRESHOLD_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB
const CHUNK_SIZE_BYTES: u64 = 256 * 1024; // 256 KiB

#[derive(Debug)]
pub struct FingerprintResult {
    pub hash: String,
    pub algo: String,
    pub mode: String,
    pub sample_chunk_bytes: i64,
    pub sample_count: i64,
}

pub fn generate_fingerprint<P: AsRef<Path>>(path: P, size_bytes: u64) -> Result<FingerprintResult, io::Error> {
    if size_bytes == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "0-byte file"));
    }

    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();

    if size_bytes < THRESHOLD_BYTES {
        // Full hash
        io::copy(&mut file, &mut hasher)?;
        
        Ok(FingerprintResult {
            hash: hasher.finalize().to_hex().to_string(),
            algo: "blake3".to_string(),
            mode: "full_v1".to_string(),
            sample_chunk_bytes: 0,
            sample_count: 0,
        })
    } else {
        // Sampled hash
        let mut buf = vec![0u8; CHUNK_SIZE_BYTES as usize];

        let offsets = [
            0,                                            // start
            (size_bytes / 3) - (CHUNK_SIZE_BYTES / 2),    // ~33%
            ((size_bytes * 2) / 3) - (CHUNK_SIZE_BYTES / 2), // ~66%
            size_bytes - CHUNK_SIZE_BYTES,                // end
        ];

        for &offset in &offsets {
            file.seek(SeekFrom::Start(offset))?;
            let mut chunk_reader = (&mut file).take(CHUNK_SIZE_BYTES);
            
            // Note: `read_exact` could fail if the file is smaller than expected,
            // but we already know size_bytes >= 32 MiB, which is > 256 KiB.
            // Still, it's safer to read up to CHUNK_SIZE_BYTES.
            let mut read_total = 0;
            while read_total < CHUNK_SIZE_BYTES as usize {
                let bytes_read = chunk_reader.read(&mut buf[read_total..])?;
                if bytes_read == 0 { break; }
                read_total += bytes_read;
            }
            hasher.update(&buf[..read_total]);
        }

        Ok(FingerprintResult {
            hash: hasher.finalize().to_hex().to_string(),
            algo: "blake3".to_string(),
            mode: "sampled_v1".to_string(),
            sample_chunk_bytes: CHUNK_SIZE_BYTES as i64,
            sample_count: 4,
        })
    }
}
