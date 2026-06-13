// =============================================================================
// 참조 (refs.rs)
// =============================================================================
//
// HEAD 와 브랜치 참조(refs/heads/<name>)를 다룬다.
// - HEAD: "ref: refs/heads/<branch>" 심볼릭 참조
// - refs/heads/<branch>: 해당 브랜치의 head 커밋 해시
//
// 파일 위치: crates/cli/src/refs.rs
// =============================================================================

use anyhow::{bail, Context, Result};

use crate::repo::Repo;

const HEAD_PREFIX: &str = "ref: refs/heads/";

/// HEAD 가 가리키는 현재 브랜치 이름
pub fn current_branch(repo: &Repo) -> Result<String> {
    let head =
        std::fs::read_to_string(repo.head_path()).context("HEAD 를 읽을 수 없습니다")?;
    let head = head.trim();
    match head.strip_prefix(HEAD_PREFIX) {
        Some(name) if !name.is_empty() => Ok(name.to_string()),
        _ => bail!("HEAD 형식을 해석할 수 없습니다: {head}"),
    }
}

/// 브랜치의 head 커밋 해시 (아직 커밋이 없으면 None)
pub fn read_branch(repo: &Repo, branch: &str) -> Result<Option<String>> {
    let path = repo.refs_heads_dir().join(branch);
    if !path.exists() {
        return Ok(None);
    }
    let hash = std::fs::read_to_string(&path)?.trim().to_string();
    Ok(if hash.is_empty() { None } else { Some(hash) })
}

/// 브랜치 head 를 커밋 해시로 갱신
pub fn update_branch(repo: &Repo, branch: &str, commit_hash: &str) -> Result<()> {
    let path = repo.refs_heads_dir().join(branch);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{commit_hash}\n"))
        .with_context(|| format!("브랜치 갱신 실패: {branch}"))?;
    Ok(())
}

/// HEAD 를 지정한 브랜치를 가리키도록 변경
pub fn set_head(repo: &Repo, branch: &str) -> Result<()> {
    std::fs::write(repo.head_path(), format!("{HEAD_PREFIX}{branch}\n"))
        .context("HEAD 갱신 실패")?;
    Ok(())
}

/// 브랜치 존재 여부
pub fn branch_exists(repo: &Repo, branch: &str) -> bool {
    repo.refs_heads_dir().join(branch).is_file()
}

/// 모든 브랜치 이름 (refs/heads 하위, 중첩 포함)
pub fn list_branches(repo: &Repo) -> Result<Vec<String>> {
    let base = repo.refs_heads_dir();
    let mut names = Vec::new();
    if base.is_dir() {
        collect_branches(&base, &base, &mut names)?;
    }
    names.sort();
    Ok(names)
}

fn collect_branches(
    base: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir).context("refs/heads 읽기 실패")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_branches(base, &path, out)?;
        } else if let Ok(rel) = path.strip_prefix(base) {
            let name = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push(name);
        }
    }
    Ok(())
}
