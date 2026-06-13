// =============================================================================
// FileBlobStorage (file_blob_storage.rs)
// =============================================================================
//
// BlobStorage 포트의 로컬 파일시스템 구현.
// 내용을 base/<repo_id>/<hash 앞2>/<hash 나머지> 에 원본 바이트로 저장한다.
//
// 파일 위치:
//   crates/server/src/repository/infrastructure/adapters/file_blob_storage.rs
// =============================================================================

use std::path::PathBuf;

use async_trait::async_trait;
use shared::error::AppError;

use crate::repository::domain::ports::BlobStorage;
use crate::repository::domain::value_objects::RepositoryId;

/// 파일시스템 기반 Blob 내용 저장소
pub struct FileBlobStorage {
    base: PathBuf,
}

impl FileBlobStorage {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }

    fn object_path(&self, repo: RepositoryId, hash: &str) -> Result<PathBuf, AppError> {
        if hash.len() < 3 {
            return Err(AppError::InvalidInput(format!("잘못된 해시: {hash}")));
        }
        let (prefix, rest) = hash.split_at(2);
        Ok(self
            .base
            .join(repo.as_uuid().to_string())
            .join(prefix)
            .join(rest))
    }
}

#[async_trait]
impl BlobStorage for FileBlobStorage {
    async fn put(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        content: &[u8],
    ) -> Result<String, AppError> {
        let path = self.object_path(repository_id, hash)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Storage(format!("디렉토리 생성 실패: {e}")))?;
        }
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| AppError::Storage(format!("blob 저장 실패: {e}")))?;
        Ok(path.to_string_lossy().into_owned())
    }

    async fn get(&self, repository_id: RepositoryId, hash: &str) -> Result<Vec<u8>, AppError> {
        let path = self.object_path(repository_id, hash)?;
        tokio::fs::read(&path)
            .await
            .map_err(|e| AppError::Storage(format!("blob 읽기 실패 ({hash}): {e}")))
    }

    async fn has(&self, repository_id: RepositoryId, hash: &str) -> Result<bool, AppError> {
        let path = self.object_path(repository_id, hash)?;
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }
}
