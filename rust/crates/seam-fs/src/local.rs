use super::trait_def::{FileSystem, FsError};
use async_trait::async_trait;

/// Local filesystem provider.
pub struct LocalFileSystem;

impl LocalFileSystem {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystem for LocalFileSystem {
    async fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        Ok(tokio::fs::read(path).await?)
    }

    async fn write(&self, path: &str, contents: &[u8]) -> Result<(), FsError> {
        Ok(tokio::fs::write(path, contents).await?)
    }

    async fn list(&self, path: &str) -> Result<Vec<String>, FsError> {
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            entries.push(entry.file_name().to_string_lossy().into_owned());
        }
        Ok(entries)
    }

    async fn exists(&self, path: &str) -> Result<bool, FsError> {
        Ok(tokio::fs::metadata(path).await.is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn read_write_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_string_lossy().into_owned();
        let fs = LocalFileSystem::new();

        fs.write(&path, b"hello").await.unwrap();
        let contents = fs.read(&path).await.unwrap();
        assert_eq!(contents, b"hello");
    }
}
