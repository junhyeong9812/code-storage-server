// =============================================================================
// Build Runner 포트 (build_runner.rs)
// =============================================================================
//
// 빌드 실행기 인터페이스.
// 구현(ShellBuildRunner)은 커밋 트리를 임시 디렉토리에 복원한 뒤 빌드 명령을
// 실행하고 로그를 파일로 남긴다.
//
// (추후 DockerBuildRunner 로 교체 가능 — 같은 포트)
//
// 파일 위치: crates/server/src/build/domain/ports/build_runner.rs
// =============================================================================

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Id;

use crate::build::domain::value_objects::BuildId;

/// 빌드 실행 결과
#[derive(Debug, Clone)]
pub struct BuildOutcome {
    /// 성공 여부 (명령 종료 코드 0)
    pub success: bool,
    /// 작성된 로그 파일 경로
    pub log_path: String,
}

/// 빌드 실행기 포트
#[async_trait]
pub trait BuildRunner: Send + Sync {
    /// 커밋을 빌드한다.
    ///
    /// - `command`: 실행할 빌드 명령. None 이면 체크아웃 루트의 `cts.build.sh` 사용.
    /// - 로그는 build_id 기준 파일에 기록한다.
    async fn run(
        &self,
        repository_id: Id,
        commit_hash: &str,
        command: Option<&str>,
        build_id: BuildId,
    ) -> Result<BuildOutcome, AppError>;
}
