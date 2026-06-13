// =============================================================================
// cts add (commands/add.rs)
// =============================================================================
//
// 파일을 스테이징 영역(index)에 추가한다.
//   cts add hello.txt        # 단일 파일
//   cts add src              # 디렉토리 재귀 추가
//   cts add .                # 작업 트리 전체 (.cts 제외)
//
// 각 파일의 내용을 blob 으로 저장하고 인덱스에 경로→해시를 기록한다.
//
// 파일 위치: crates/cli/src/commands/add.rs
// =============================================================================

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::index::{Index, IndexEntry};
use crate::objects;
use crate::repo::{Repo, CTS_DIR};

pub fn run(files: Vec<String>) -> Result<()> {
    if files.is_empty() {
        bail!("추가할 파일을 지정하세요: cts add <file>...");
    }

    let repo = Repo::discover()?;
    let root = std::fs::canonicalize(&repo.root).context("저장소 루트 경로 해석 실패")?;
    let mut index = Index::load(&repo)?;

    let mut added = 0usize;
    for file in &files {
        let abs = std::fs::canonicalize(Path::new(file))
            .with_context(|| format!("경로를 찾을 수 없습니다: {file}"))?;
        if abs.is_dir() {
            added += add_dir(&repo, &root, &mut index, &abs)?;
        } else {
            add_file(&repo, &root, &mut index, &abs)?;
            added += 1;
        }
    }

    index.save(&repo)?;
    println!("{added}개 파일을 스테이징했습니다.");
    Ok(())
}

/// 디렉토리를 재귀적으로 스테이징 (.cts 제외)
fn add_dir(repo: &Repo, root: &Path, index: &mut Index, dir: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("디렉토리를 읽을 수 없습니다: {}", dir.display()))?
    {
        let entry = entry?;
        if entry.file_name() == CTS_DIR {
            continue; // 메타 디렉토리 제외
        }
        let path = entry.path();
        if path.is_dir() {
            count += add_dir(repo, root, index, &path)?;
        } else {
            add_file(repo, root, index, &path)?;
            count += 1;
        }
    }
    Ok(count)
}

/// 단일 파일을 blob 으로 저장하고 인덱스에 기록
fn add_file(repo: &Repo, root: &Path, index: &mut Index, abs: &Path) -> Result<()> {
    let content =
        std::fs::read(abs).with_context(|| format!("파일을 읽을 수 없습니다: {}", abs.display()))?;
    let hash = objects::write_blob(repo, &content)?;
    let rel = rel_path(root, abs)?;
    index.upsert(IndexEntry {
        path: rel,
        hash,
        mode: file_mode(abs),
        size: content.len() as u64,
    });
    Ok(())
}

/// 저장소 루트 기준 상대 경로(슬래시 구분) 계산
fn rel_path(root: &Path, abs: &Path) -> Result<String> {
    let rel: &Path = abs
        .strip_prefix(root)
        .with_context(|| format!("저장소 밖의 경로입니다: {}", abs.display()))?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        bail!("파일 경로가 비어 있습니다");
    }
    Ok(parts.join("/"))
}

/// 파일 모드 추론 (실행 비트 → 100755, 그 외 100644)
fn file_mode(abs: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(abs) {
            if meta.permissions().mode() & 0o111 != 0 {
                return "100755".to_string();
            }
        }
    }
    let _ = abs;
    "100644".to_string()
}
