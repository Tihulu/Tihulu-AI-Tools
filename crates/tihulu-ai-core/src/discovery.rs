//! Consent-gated, read-only directory inventory. This is not an inference
//! runtime, a model installer or an application-specific Jan parser.
//!
//! It inspects filenames, file types and sizes inside a user-selected scope;
//! it never reads the model payload, follows interior symlinks or writes files.

use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    Gguf,
    Onnx,
    Safetensors,
    HailoHef,
    WhisperGgml,
    Tflite,
}

impl ModelFormat {
    /// Conservative filename-only detector. Unknown binary files are ignored.
    /// This says nothing about the model's validity, license or task capability.
    pub fn candidate_from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "gguf" => Some(Self::Gguf),
            "onnx" => Some(Self::Onnx),
            "safetensors" => Some(Self::Safetensors),
            "hef" => Some(Self::HailoHef),
            "tflite" => Some(Self::Tflite),
            "bin" => {
                let name = path.file_name()?.to_str()?.to_ascii_lowercase();
                if name.starts_with("ggml-") {
                    Some(Self::WhisperGgml)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCandidate {
    pub path: PathBuf,
    pub format: ModelFormat,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanPolicy {
    /// Must be explicitly enabled by the caller for an approved user scope.
    pub allow_read: bool,
    /// Maximum number of directory levels explored below the requested root.
    pub max_depth: usize,
    /// Limits directory walk work; results may then be incomplete.
    pub max_entries: usize,
    /// Upper bound for the number of candidate entries returned.
    pub max_models: usize,
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            allow_read: false,
            max_depth: 4,
            max_entries: 10_000,
            max_models: 500,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub models: Vec<ModelCandidate>,
    pub scanned_entries: usize,
    pub skipped_symlinks: usize,
    pub truncated: bool,
}

/// Read model *candidate metadata* under the explicitly selected directory.
///
/// The caller must ask the user for read access separately. A symlink supplied
/// as the root is resolved and treated as the explicitly selected scope; all
/// symlinks encountered below that root are skipped. The result is not a
/// verified model registry and contains no model owner or inference promises.
pub fn scan_directory(root: &Path, policy: ScanPolicy) -> io::Result<ScanReport> {
    if !policy.allow_read {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "read-only scan needs explicit user consent",
        ));
    }

    let canonical_root = root.canonicalize()?;
    if !canonical_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scan scope must be a directory",
        ));
    }

    let mut report = ScanReport::default();
    let mut directories = VecDeque::new();
    directories.push_back((canonical_root, 0usize));

    'walk: while let Some((directory, depth)) = directories.pop_front() {
        let entries = fs::read_dir(&directory)?;
        for entry in entries {
            if report.scanned_entries >= policy.max_entries
                || report.models.len() >= policy.max_models
            {
                report.truncated = true;
                break 'walk;
            }
            let entry = entry?;
            report.scanned_entries += 1;
            let file_type = entry.file_type()?;

            if file_type.is_symlink() {
                report.skipped_symlinks += 1;
                continue;
            }

            if file_type.is_dir() {
                if depth < policy.max_depth {
                    directories.push_back((entry.path(), depth + 1));
                }
            } else if file_type.is_file() {
                if let Some(format) = ModelFormat::candidate_from_path(&entry.path()) {
                    let size_bytes = entry.metadata()?.len();
                    report.models.push(ModelCandidate {
                        path: entry.path(),
                        format,
                        size_bytes,
                    });
                }
            }
        }
    }

    report.models.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    fn temp_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tihulu-core-test-{}-{unique}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temp test directory");
        root
    }

    #[test]
    fn scanning_requires_explicit_permission() {
        let root = temp_root();
        let result = scan_directory(&root, ScanPolicy::default());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovers_known_formats_without_claiming_they_work() {
        let root = temp_root();
        let sub = root.join("models");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("example.GGUF"), b"unverified").unwrap();
        fs::write(sub.join("ggml-small.bin"), b"unverified").unwrap();
        fs::write(sub.join("model.hef"), b"unverified").unwrap();
        fs::write(sub.join("notes.bin"), b"not a model").unwrap();
        fs::write(sub.join("chat-history.txt"), b"private").unwrap();

        let report = scan_directory(
            &root,
            ScanPolicy {
                allow_read: true,
                ..ScanPolicy::default()
            },
        )
        .unwrap();

        assert_eq!(report.models.len(), 3);
        assert!(report.models.iter().any(|m| m.format == ModelFormat::Gguf));
        assert!(report
            .models
            .iter()
            .any(|m| m.format == ModelFormat::HailoHef));
        assert!(report
            .models
            .iter()
            .any(|m| m.format == ModelFormat::WhisperGgml));
        assert!(!report.truncated);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounded_scan_reports_incomplete_inventory() {
        let root = temp_root();
        for index in 0..8 {
            fs::write(root.join(format!("{index}.gguf")), b"x").unwrap();
        }
        let report = scan_directory(
            &root,
            ScanPolicy {
                allow_read: true,
                max_entries: 2,
                ..ScanPolicy::default()
            },
        )
        .unwrap();
        assert!(report.truncated);
        assert_eq!(report.scanned_entries, 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_symlinks_beneath_selected_root() {
        use std::os::unix::fs::symlink;

        let root = temp_root();
        let outside = temp_root();
        fs::write(outside.join("outside.gguf"), b"do-not-open").unwrap();
        symlink(&outside, root.join("outside-link")).unwrap();

        let report = scan_directory(
            &root,
            ScanPolicy {
                allow_read: true,
                ..ScanPolicy::default()
            },
        )
        .unwrap();

        assert!(report.models.is_empty());
        assert_eq!(report.skipped_symlinks, 1);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
