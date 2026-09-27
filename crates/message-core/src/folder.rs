use anyhow::{Context, Result, bail};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

const MAGIC_HEADER: &[u8; 6] = b"IRDIR1";

/// Recursively packs a directory into a single archive file.
pub fn pack_directory_to_file(source_dir: &Path, out_file_path: &Path) -> Result<u64> {
    if !source_dir.is_dir() {
        bail!("Source path is not a directory: {:?}", source_dir);
    }

    if let Some(parent) = out_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let out_file = File::create(out_file_path)
        .with_context(|| format!("Failed creating output archive file: {:?}", out_file_path))?;
    let mut writer = BufWriter::new(out_file);

    // 1. Write magic header
    writer.write_all(MAGIC_HEADER)?;

    // 2. Collect all entries
    let mut entries = Vec::new();
    collect_directory_entries(source_dir, Path::new(""), &mut entries)?;

    // 3. Write entries count (u32)
    let count = entries.len() as u32;
    writer.write_all(&count.to_be_bytes())?;

    // 4. Write each entry
    for (rel_path, is_dir) in entries {
        let path_str = rel_path.to_string_lossy().replace('\\', "/");
        let path_bytes = path_str.as_bytes();
        let path_len = path_bytes.len() as u16;

        writer.write_all(&path_len.to_be_bytes())?;
        writer.write_all(path_bytes)?;
        writer.write_all(&[if is_dir { 1u8 } else { 0u8 }])?;

        if !is_dir {
            let full_path = source_dir.join(&rel_path);
            let file_size = fs::metadata(&full_path)?.len();
            writer.write_all(&file_size.to_be_bytes())?;

            let mut f = BufReader::new(File::open(&full_path)?);
            let mut buffer = [0u8; 64 * 1024];
            let mut written = 0u64;
            while written < file_size {
                let to_read = std::cmp::min(buffer.len() as u64, file_size - written) as usize;
                f.read_exact(&mut buffer[..to_read])?;
                writer.write_all(&buffer[..to_read])?;
                written += to_read as u64;
            }
        }
    }

    writer.flush()?;
    let total_size = fs::metadata(out_file_path)?.len();
    Ok(total_size)
}

/// Recursively collects all relative paths within a directory.
fn collect_directory_entries(
    base: &Path,
    rel: &Path,
    out: &mut Vec<(PathBuf, bool)>,
) -> Result<()> {
    let current_dir = base.join(rel);
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_rel = rel.join(entry.file_name());

        if file_type.is_dir() {
            out.push((entry_rel.clone(), true));
            collect_directory_entries(base, &entry_rel, out)?;
        } else if file_type.is_file() {
            out.push((entry_rel, false));
        }
    }
    Ok(())
}

/// Unpacks an archive file into the destination directory.
pub fn unpack_directory_from_file(archive_file_path: &Path, destination_dir: &Path) -> Result<()> {
    let in_file = File::open(archive_file_path)
        .with_context(|| format!("Failed opening archive file: {:?}", archive_file_path))?;
    let mut reader = BufReader::new(in_file);

    // 1. Verify magic header
    let mut header = [0u8; 6];
    reader.read_exact(&mut header)?;
    if &header != MAGIC_HEADER {
        bail!("Invalid folder archive magic header");
    }

    // 2. Read entries count
    let mut count_bytes = [0u8; 4];
    reader.read_exact(&mut count_bytes)?;
    let count = u32::from_be_bytes(count_bytes);

    fs::create_dir_all(destination_dir)?;

    // 3. Extract each entry
    for _ in 0..count {
        let mut path_len_bytes = [0u8; 2];
        reader.read_exact(&mut path_len_bytes)?;
        let path_len = u16::from_be_bytes(path_len_bytes) as usize;

        let mut path_bytes = vec![0u8; path_len];
        reader.read_exact(&mut path_bytes)?;
        let rel_path_str = String::from_utf8(path_bytes)?;

        let mut is_dir_byte = [0u8; 1];
        reader.read_exact(&mut is_dir_byte)?;
        let is_dir = is_dir_byte[0] == 1;

        // Security check: validate relative path against directory traversal
        let sanitized_rel = sanitize_relative_path(&rel_path_str)?;
        let target_path = destination_dir.join(sanitized_rel);

        if is_dir {
            fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut size_bytes = [0u8; 8];
            reader.read_exact(&mut size_bytes)?;
            let file_size = u64::from_be_bytes(size_bytes);

            let mut out_file = BufWriter::new(File::create(&target_path)?);
            let mut buffer = [0u8; 64 * 1024];
            let mut read_bytes = 0u64;

            while read_bytes < file_size {
                let to_read = std::cmp::min(buffer.len() as u64, file_size - read_bytes) as usize;
                reader.read_exact(&mut buffer[..to_read])?;
                out_file.write_all(&buffer[..to_read])?;
                read_bytes += to_read as u64;
            }
            out_file.flush()?;
        }
    }

    Ok(())
}

/// Prevents directory traversal attacks by validating that the path is purely relative.
fn sanitize_relative_path(path_str: &str) -> Result<PathBuf> {
    let p = Path::new(path_str);
    for component in p.components() {
        match component {
            Component::Normal(_) => {}
            _ => bail!(
                "Path traversal attempt detected in archive path: {}",
                path_str
            ),
        }
    }
    Ok(p.to_path_buf())
}

/// Computes the 32-byte BLAKE3 hash of a file or data stream in hex format.
pub fn compute_file_blake3(path: &Path) -> Result<(String, u64)> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut total_size = 0u64;

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        total_size += n as u64;
    }

    let hash_hex = hasher.finalize().to_hex().to_string();
    Ok((hash_hex, total_size))
}

/// Computes BLAKE3 hash of an in-memory byte slice.
pub fn compute_bytes_blake3(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Detects standard MIME types based on file extension.
pub fn detect_mime_type(file_name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "application/x-directory";
    }

    let lower = file_name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",

        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "aac" => "audio/aac",

        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",

        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "tar" | "gz" | "tgz" => "application/gzip",
        "txt" | "md" | "rs" | "json" | "toml" | "log" | "xml" | "csv" | "yaml" | "yml" => {
            "text/plain"
        }
        "html" | "htm" => "text/html",
        "doc" | "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" | "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" | "pptx" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_pack_and_unpack_directory() -> Result<()> {
        let temp = tempdir()?;
        let src_dir = temp.path().join("source");
        fs::create_dir_all(src_dir.join("sub1/sub2"))?;
        fs::write(src_dir.join("hello.txt"), b"Hello World")?;
        fs::write(
            src_dir.join("sub1/nested.json"),
            b"{\"key\": \"value\", \"numbers\": [1,2,3]}",
        )?;
        fs::write(
            src_dir.join("sub1/sub2/deep.bin"),
            vec![0xCA, 0xFE, 0xBA, 0xBE],
        )?;

        let archive_path = temp.path().join("test.irdir");
        let packed_size = pack_directory_to_file(&src_dir, &archive_path)?;
        assert!(packed_size > 0);

        let (hash, size) = compute_file_blake3(&archive_path)?;
        assert_eq!(size, packed_size);
        assert!(!hash.is_empty());

        let dst_dir = temp.path().join("extracted");
        unpack_directory_from_file(&archive_path, &dst_dir)?;

        assert_eq!(
            fs::read(dst_dir.join("hello.txt"))?,
            b"Hello World".to_vec()
        );
        assert_eq!(
            fs::read(dst_dir.join("sub1/nested.json"))?,
            b"{\"key\": \"value\", \"numbers\": [1,2,3]}".to_vec()
        );
        assert_eq!(
            fs::read(dst_dir.join("sub1/sub2/deep.bin"))?,
            vec![0xCA, 0xFE, 0xBA, 0xBE]
        );

        Ok(())
    }

    #[test]
    fn test_mime_detection() {
        assert_eq!(detect_mime_type("photo.jpg", false), "image/jpeg");
        assert_eq!(detect_mime_type("recording.mp3", false), "audio/mpeg");
        assert_eq!(detect_mime_type("clip.mp4", false), "video/mp4");
        assert_eq!(detect_mime_type("paper.pdf", false), "application/pdf");
        assert_eq!(
            detect_mime_type("MyFolder", true),
            "application/x-directory"
        );
    }
}
