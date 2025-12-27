// ============================================
// Repository 포트 (Ports)
// ============================================
// 헥사고날 아키텍처의 "포트"
// = 인터페이스 = Rust에서는 trait
//
// 도메인이 외부와 소통하는 방법을 정의
// 실제 구현은 infrastructure 레이어에서
//
// 포트 종류:
//   - Inbound Port (Driving): 외부에서 도메인을 호출
//     → Use Case가 이 역할을 함
//   - Outbound Port (Driven): 도메인이 외부를 호출
//     → Repository, Gateway 등
//
// 이 모듈에 정의될 포트들 (Outbound):
//   - RepositoryRepository: 저장소 메타데이터 저장/조회 (DB)
//   - GitPort: Git 작업 (git2-rs)
//
// 예시 (나중에 구현):
//
// use async_trait::async_trait;
//
// #[async_trait]
// pub trait RepositoryRepository {
//     async fn save(&self, repo: &Repository) -> Result<(), Error>;
//     async fn find_by_id(&self, id: &RepositoryId) -> Result<Option<Repository>, Error>;
//     async fn find_all(&self) -> Result<Vec<Repository>, Error>;
//     async fn delete(&self, id: &RepositoryId) -> Result<(), Error>;
// }
//
// pub trait GitPort {
//     fn init(&self, path: &Path) -> Result<(), Error>;
//     fn clone(&self, url: &str, path: &Path) -> Result<(), Error>;
//     fn get_commits(&self, path: &Path, branch: &str) -> Result<Vec<Commit>, Error>;
// }

mod repository_repository;
mod git_repository;