// =============================================================================
// Blob Storage 포트 (blob_storage.rs)
// =============================================================================
//
// Blob "내용"의 영속화 인터페이스. (메타데이터는 ObjectRepository/DB 담당)
// 구현은 로컬 파일시스템(FileBlobStorage), 추후 S3 등으로 교체 가능.
//
// 파일 위치: crates/server/src/repository/domain/ports/blob_storage.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;

use crate::repository::domain::value_objects::RepositoryId;

/// Blob 내용 저장소 포트
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// 내용을 저장하고 저장 경로 문자열을 반환한다.
    async fn put(
        &self,
        repository_id: RepositoryId,
        hash: &str,
        content: &[u8],
    ) -> Result<String, AppError>;

    /// 내용을 읽는다.
    async fn get(&self, repository_id: RepositoryId, hash: &str) -> Result<Vec<u8>, AppError>;

    /// 내용 존재 여부
    async fn has(&self, repository_id: RepositoryId, hash: &str) -> Result<bool, AppError>;
}
