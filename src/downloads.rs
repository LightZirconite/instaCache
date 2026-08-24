//! Where a downloaded file goes, and under what name.
//!
//! Pure path arithmetic, deliberately independent of any toolkit: the name
//! arrives from a remote server, so it is the one piece of download handling
//! that has to be right, and it is therefore the one piece that is unit
//! tested.

use std::path::{Path, PathBuf};

/// The directory downloads land in: the XDG download directory, the home
/// directory if there is none, and the working directory if there is no home
/// either.
pub fn directory() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Full destination for a server-suggested name, creating the directory if it
/// is missing. `None` means the directory could not be created.
pub fn destination_for(suggested: &str) -> Option<PathBuf> {
    let directory = directory();
    std::fs::create_dir_all(&directory).ok()?;
    Some(unique_destination(&directory, suggested))
}

/// `foo.jpg` -> `foo.jpg`, `foo (1).jpg`, `foo (2).jpg`, …
pub fn unique_destination(directory: &Path, suggested: &str) -> PathBuf {
    let name = sanitize_file_name(suggested);
    let candidate = directory.join(&name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = directory.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(name)
}

/// The suggested name comes from the remote server, so path separators and
/// traversal segments are stripped before it touches the filesystem.
pub fn sanitize_file_name(suggested: &str) -> String {
    let base = file_name_of(suggested);
    let cleaned: String = base
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0'))
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        "download".to_string()
    } else {
        cleaned.chars().take(200).collect()
    }
}

pub fn file_name_of(value: &str) -> String {
    value
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty() && *part != "." && *part != "..")
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_path_traversal_from_suggested_names() {
        assert_eq!(sanitize_file_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_file_name("/tmp/a.jpg"), "a.jpg");
        assert_eq!(sanitize_file_name(""), "download");
        assert_eq!(sanitize_file_name("   "), "download");
        assert_eq!(sanitize_file_name(".bashrc"), "bashrc");
    }

    #[test]
    fn deduplicates_download_names() {
        let dir = std::env::temp_dir().join(format!("instacache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.jpg"), b"x").unwrap();

        assert_eq!(
            unique_destination(&dir, "photo.jpg"),
            dir.join("photo (1).jpg")
        );
        assert_eq!(unique_destination(&dir, "other.jpg"), dir.join("other.jpg"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
