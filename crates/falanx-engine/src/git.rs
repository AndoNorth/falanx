use std::path::PathBuf;

pub struct Diff(pub String);

pub enum ReviewTarget {
    GitRange(String),
    File(PathBuf),
}

impl ReviewTarget {
    pub fn extract_diff(&self) -> anyhow::Result<Diff> {
        match self {
            ReviewTarget::File(path) => {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("failed to read {}: {}", path.display(), e))?;
                Ok(Diff(content))
            }
            ReviewTarget::GitRange(range) => {
                let output = std::process::Command::new("git")
                    .args(["diff", range])
                    .output()
                    .map_err(|e| anyhow::anyhow!("failed to run git diff: {}", e))?;
                if !output.status.success() {
                    anyhow::bail!(
                        "git diff {} failed: {}",
                        range,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                Ok(Diff(String::from_utf8(output.stdout)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_target_reads_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.rs");
        std::fs::write(&path, "fn main() {}").unwrap();

        let target = ReviewTarget::File(path);
        let diff = target.extract_diff().unwrap();
        assert_eq!(diff.0, "fn main() {}");
    }

    #[test]
    fn file_target_missing_file_returns_error() {
        let target = ReviewTarget::File(PathBuf::from("/nonexistent/path/file.rs"));
        assert!(target.extract_diff().is_err());
    }
}
