use std::path::{Path, PathBuf};

pub fn validate_path(root: &Path, requested: &str) -> Result<PathBuf, SecurityError> {
    let decoded = percent_encoding::percent_decode_str(requested)
        .decode_utf8()
        .map_err(|_| SecurityError::InvalidEncoding)?;

    let cleaned = decoded.trim_start_matches('/');

    if cleaned.split('/').any(|seg| seg == ".." || seg == ".") {
        return Err(SecurityError::TraversalAttempt);
    }

    let full_path = if cleaned.is_empty() {
        root.to_path_buf()
    } else {
        root.join(cleaned)
    };

    let canonical = if full_path.exists() {
        full_path
            .canonicalize()
            .map_err(|_| SecurityError::NotFound)?
    } else {
        return Err(SecurityError::NotFound);
    };

    if !canonical.starts_with(root) {
        return Err(SecurityError::TraversalAttempt);
    }

    Ok(canonical)
}

#[derive(Debug, PartialEq)]
pub enum SecurityError {
    TraversalAttempt,
    InvalidEncoding,
    NotFound,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TraversalAttempt => write!(f, "access denied"),
            Self::InvalidEncoding => write!(f, "invalid path encoding"),
            Self::NotFound => write!(f, "not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "hello").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/inner.txt"), "inner").unwrap();
        dir
    }

    #[test]
    fn test_valid_file() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/file.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_subdir() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/subdir/inner.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_traversal_dotdot() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/../etc/passwd");
        assert_eq!(result.unwrap_err(), SecurityError::TraversalAttempt);
    }

    #[test]
    fn test_traversal_encoded() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/%2e%2e/etc/passwd");
        assert_eq!(result.unwrap_err(), SecurityError::TraversalAttempt);
    }

    #[test]
    fn test_not_found() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/nonexistent.txt");
        assert_eq!(result.unwrap_err(), SecurityError::NotFound);
    }

    #[test]
    fn test_root_path() {
        let dir = setup_test_dir();
        let root = dir.path().canonicalize().unwrap();
        let result = validate_path(&root, "/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), root);
    }
}
