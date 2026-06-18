use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Recursively walk a directory, yielding paths matching any of the given extensions.
/// Extensions should be provided without a leading dot, e.g. &["mp3", "flac"].
pub fn scan_directory(
    root: &Path,
    extensions: &[&str],
) -> impl Iterator<Item = io::Result<PathBuf>> + 'static {
    let ext_owned: Vec<String> = extensions.iter().map(|e| e.to_string()).collect();
    let walker = walkdir::WalkDir::new(root).follow_links(false).into_iter();
    walker.filter_map(move |entry| {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => return Some(Err(io::Error::new(io::ErrorKind::Other, e))),
        };
        if !entry.file_type().is_file() {
            return None;
        }
        let path = entry.into_path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => return None,
        };
        if ext_owned.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            Some(Ok(path))
        } else {
            None
        }
    })
}

/// Ensure the cache subdirectory exists inside `base`, returning `base/cache/`.
pub fn ensure_cache_dir(base: &Path) -> io::Result<PathBuf> {
    let cache_path = base.join("cache");
    fs::create_dir_all(&cache_path)?;
    Ok(cache_path)
}

/// Read the first and last `chunk_size` bytes of a file.
/// If the file is smaller than 2 * chunk_size, reads the whole file and splits.
pub fn read_first_last_chunks(path: &Path, chunk_size: usize) -> io::Result<(Vec<u8>, Vec<u8>)> {
    use std::io::Read;
    use std::io::Seek;

    let meta = fs::metadata(path)?;
    let file_len = meta.len() as usize;

    if file_len <= 2 * chunk_size {
        let mut buf = Vec::with_capacity(file_len);
        fs::File::open(path)?.read_to_end(&mut buf)?;
        let mid = buf.len() / 2;
        let first = buf[..mid].to_vec();
        let last = buf[mid..].to_vec();
        return Ok((first, last));
    }

    let mut file = fs::File::open(path)?;

    let mut first = vec![0u8; chunk_size];
    file.read_exact(&mut first)?;

    file.seek(std::io::SeekFrom::End(-(chunk_size as i64)))?;
    let mut last = vec![0u8; chunk_size];
    file.read_exact(&mut last)?;

    Ok((first, last))
}
