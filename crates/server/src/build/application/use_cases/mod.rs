// =============================================================================
// Build 유스케이스 (use_cases/mod.rs)
// =============================================================================
//
// - run_build: 빌드 생성 → 실행(인라인) → 상태 기록
// - get_build / list_builds / get_build_log
//
// 참고: run_build 는 실행 완료까지 await 한다(데모/검증 단순화). 실제 CI 는
// 즉시 pending 을 반환하고 백그라운드로 실행할 것.
//
// 파일 위치: crates/server/src/build/application/use_cases/mod.rs
// =============================================================================

use shared::error::AppError;
use shared::types::{now, Id};

use crate::build::domain::entities::Build;
use crate::build::domain::ports::{BuildRepository, BuildRunner};
use crate::build::domain::value_objects::{BuildId, BuildStatus};

/// 커밋에 대한 빌드를 생성하고 실행한다.
pub async fn run_build(
    builds: &dyn BuildRepository,
    runner: &dyn BuildRunner,
    repository_id: Id,
    commit_hash: &str,
    command: Option<&str>,
) -> Result<Build, AppError> {
    // 1. pending 빌드 생성 (커밋 해시 해석 포함)
    let build = builds.create(repository_id, commit_hash).await?;

    // 2. running
    builds.mark_running(build.id(), now()).await?;

    // 3. 실행
    let outcome = runner
        .run(repository_id, commit_hash, command, build.id())
        .await?;

    // 4. 종료 상태 기록
    let status = if outcome.success {
        BuildStatus::Success
    } else {
        BuildStatus::Failed
    };
    builds
        .mark_finished(build.id(), status, now(), &outcome.log_path)
        .await?;

    builds
        .find_by_id(build.id())
        .await?
        .ok_or_else(|| AppError::Internal("빌드 생성 후 조회 실패".into()))
}

/// 빌드 조회
pub async fn get_build(builds: &dyn BuildRepository, id: BuildId) -> Result<Build, AppError> {
    builds
        .find_by_id(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("빌드 {id}")))
}

/// 저장소의 빌드 목록
pub async fn list_builds(
    builds: &dyn BuildRepository,
    repository_id: Id,
) -> Result<Vec<Build>, AppError> {
    builds.list_by_repository(repository_id).await
}

/// 빌드 로그 조회 (로그 파일 내용)
pub async fn get_build_log(builds: &dyn BuildRepository, id: BuildId) -> Result<String, AppError> {
    let build = get_build(builds, id).await?;
    match build.log_path() {
        Some(path) => tokio::fs::read_to_string(path)
            .await
            .map_err(|e| AppError::Storage(format!("로그 읽기 실패: {e}"))),
        None => Ok(String::new()),
    }
}
