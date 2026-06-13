// =============================================================================
// ShellBuildRunner (shell_build_runner.rs)
// =============================================================================
//
// BuildRunner 포트의 로컬 셸 구현.
//
// 동작:
// 1. 커밋의 트리를 임시 작업 디렉토리에 복원(materialize)
// 2. 빌드 명령 결정: 요청 command, 없으면 루트의 cts.build.sh
// 3. `sh -c <command>` 실행 (cwd = 작업 디렉토리), stdout/stderr 캡처
// 4. 로그를 <logs_dir>/<build_id>.log 에 기록, 작업 디렉토리 정리
//
// 주의: 빌드 명령은 샌드박스 없이 실행된다(신뢰 경계 = 서버). 격리가 필요하면
// 같은 포트로 DockerBuildRunner 를 구현하면 된다.
//
// 파일 위치:
//   crates/server/src/build/infrastructure/adapters/shell_build_runner.rs
// =============================================================================

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use shared::error::AppError;
use shared::types::Id;

use crate::build::domain::ports::{BuildOutcome, BuildRunner};
use crate::build::domain::value_objects::BuildId;
use crate::repository::domain::ports::{BlobStorage, ObjectRepository};
use crate::repository::domain::value_objects::RepositoryId;

/// 기본 빌드 스크립트 파일명
const BUILD_SCRIPT: &str = "cts.build.sh";

pub struct ShellBuildRunner {
    objects: Arc<dyn ObjectRepository>,
    blobs: Arc<dyn BlobStorage>,
    logs_dir: PathBuf,
    work_dir: PathBuf,
}

impl ShellBuildRunner {
    pub fn new(
        objects: Arc<dyn ObjectRepository>,
        blobs: Arc<dyn BlobStorage>,
        logs_dir: impl Into<PathBuf>,
        work_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            objects,
            blobs,
            logs_dir: logs_dir.into(),
            work_dir: work_dir.into(),
        }
    }

    /// 트리를 디렉토리에 재귀적으로 복원
    fn materialize<'a>(
        &'a self,
        repo: RepositoryId,
        tree_hash: &'a str,
        dir: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
            let entries = self.objects.get_tree_entries(repo, tree_hash).await?;
            for entry in entries {
                let target = dir.join(&entry.name);
                match entry.object_type.as_str() {
                    "blob" => {
                        let content = self.blobs.get(repo, &entry.child_hash).await?;
                        tokio::fs::write(&target, &content)
                            .await
                            .map_err(|e| AppError::Storage(format!("파일 복원 실패: {e}")))?;
                        set_mode(&target, &entry.mode);
                    }
                    "tree" => {
                        tokio::fs::create_dir_all(&target)
                            .await
                            .map_err(|e| AppError::Storage(e.to_string()))?;
                        self.materialize(repo, &entry.child_hash, &target).await?;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }
}

#[async_trait]
impl BuildRunner for ShellBuildRunner {
    async fn run(
        &self,
        repository_id: Id,
        commit_hash: &str,
        command: Option<&str>,
        build_id: BuildId,
    ) -> Result<BuildOutcome, AppError> {
        let repo = RepositoryId::from_uuid(repository_id);

        let commit = self
            .objects
            .get_commit(repo, commit_hash)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("커밋 {commit_hash}")))?;

        // 작업 디렉토리 준비
        let workdir = self.work_dir.join(build_id.to_string());
        let _ = tokio::fs::remove_dir_all(&workdir).await;
        tokio::fs::create_dir_all(&workdir)
            .await
            .map_err(|e| AppError::Storage(format!("작업 디렉토리 생성 실패: {e}")))?;

        // 트리 복원
        self.materialize(repo, &commit.tree_hash, &workdir).await?;

        // 빌드 명령 결정
        let script_path = workdir.join(BUILD_SCRIPT);
        let resolved_command = match command {
            Some(c) => Some(c.to_string()),
            None if script_path.is_file() => Some(format!("sh {BUILD_SCRIPT}")),
            None => None,
        };

        // 로그 디렉토리 준비
        tokio::fs::create_dir_all(&self.logs_dir)
            .await
            .map_err(|e| AppError::Storage(format!("로그 디렉토리 생성 실패: {e}")))?;
        let log_path = self.logs_dir.join(format!("{build_id}.log"));

        let (success, log) = match resolved_command {
            None => (
                false,
                format!(
                    "빌드 명령이 없습니다. 요청에 command 를 주거나 저장소 루트에 {BUILD_SCRIPT} 를 두세요.\n"
                ),
            ),
            Some(cmd) => {
                let output = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .current_dir(&workdir)
                    .output()
                    .await
                    .map_err(|e| AppError::Storage(format!("빌드 실행 실패: {e}")))?;

                let mut log = String::new();
                log.push_str(&format!("$ {cmd}\n\n"));
                log.push_str("--- stdout ---\n");
                log.push_str(&String::from_utf8_lossy(&output.stdout));
                log.push_str("\n--- stderr ---\n");
                log.push_str(&String::from_utf8_lossy(&output.stderr));
                log.push_str(&format!("\n--- exit: {} ---\n", output.status));
                (output.status.success(), log)
            }
        };

        tokio::fs::write(&log_path, log)
            .await
            .map_err(|e| AppError::Storage(format!("로그 기록 실패: {e}")))?;

        // 작업 디렉토리 정리
        let _ = tokio::fs::remove_dir_all(&workdir).await;

        Ok(BuildOutcome {
            success,
            log_path: log_path.to_string_lossy().into_owned(),
        })
    }
}

/// 실행 모드 복원
fn set_mode(path: &Path, mode: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if mode == "100755" {
            if let Ok(meta) = std::fs::metadata(path) {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                let _ = std::fs::set_permissions(path, perm);
            }
        }
    }
    let _ = (path, mode);
}
