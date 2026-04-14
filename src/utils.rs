use std::path::{Path, PathBuf};
use anyhow::{bail, Result};

/// Validates that a resolved path is inside the expected parent directory.
/// Same canonicalize + starts_with pattern as KnowledgeStore::resolve_file.
pub fn assert_path_within(file_path: &Path, parent: &Path) -> Result<()> {
    let canonical_parent = parent.canonicalize()?;

    let resolved = if file_path.exists() {
        file_path.canonicalize()?
    } else {
        let absolute = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            parent.join(file_path)
        };

        let mut normalized = PathBuf::new();
        for component in absolute.components() {
            match component {
                std::path::Component::ParentDir => { normalized.pop(); }
                std::path::Component::CurDir => {}
                c => normalized.push(c),
            }
        }

        match normalized.strip_prefix(parent) {
            Ok(suffix) => canonical_parent.join(suffix),
            Err(_) => normalized,
        }
    };

    if !resolved.starts_with(&canonical_parent) {
        bail!(
            "Path traversal detected: {} is outside {}",
            resolved.display(),
            canonical_parent.display()
        )
    }
    Ok(())
}