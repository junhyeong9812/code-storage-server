// =============================================================================
// 객체 저장소 (objects.rs)
// =============================================================================
//
// `.cts/objects/` 에 blob/tree/commit 객체를 내용 주소 지정(content-addressed)
// 방식으로 저장한다. (Git 의 .git/objects 와 동일한 아이디어)
//
// 저장 포맷:
//   경로:    objects/<해시 앞 2자>/<해시 나머지>
//   내용:    zlib( "<type> <len>\0<body>" )
//
// - blob 의 body 는 원본 파일 바이트이며, 이때 저장 바이트열의 해시가
//   곧 객체 id (Blob::hash 와 동일한 "blob <size>\0<content>" 규칙).
// - tree/commit 의 body 는 직렬화(JSON)이고, 객체 id 는 core 의 해시 규칙을
//   따른다(저장 포맷과 분리). 헤더의 type 태그는 읽을 때 종류 판별용.
//
// 파일 위치: crates/cli/src/objects.rs
// =============================================================================

use anyhow::{bail, Context, Result};
use cts_core::{compress, decompress, Blob, Commit, Tree};

use crate::repo::Repo;

/// 객체를 압축 저장한다. 같은 해시가 이미 있으면 건너뛴다(불변/중복제거).
pub fn write_object(repo: &Repo, obj_type: &str, id: &str, body: &[u8]) -> Result<()> {
    if id.len() < 3 {
        bail!("객체 해시가 올바르지 않습니다: {id}");
    }

    let header = format!("{obj_type} {}\0", body.len());
    let mut payload = header.into_bytes();
    payload.extend_from_slice(body);
    let compressed = compress(&payload).context("객체 압축 실패")?;

    let (prefix, rest) = id.split_at(2);
    let obj_dir = repo.objects_dir().join(prefix);
    std::fs::create_dir_all(&obj_dir)?;
    let path = obj_dir.join(rest);
    if !path.exists() {
        std::fs::write(&path, compressed)
            .with_context(|| format!("객체 저장 실패: {}", path.display()))?;
    }
    Ok(())
}

/// 객체를 읽어 (type, body) 반환
pub fn read_object(repo: &Repo, id: &str) -> Result<(String, Vec<u8>)> {
    if id.len() < 3 {
        bail!("객체 해시가 올바르지 않습니다: {id}");
    }
    let (prefix, rest) = id.split_at(2);
    let path = repo.objects_dir().join(prefix).join(rest);
    let compressed =
        std::fs::read(&path).with_context(|| format!("객체를 찾을 수 없습니다: {id}"))?;
    let payload = decompress(&compressed).context("객체 압축 해제 실패")?;

    let nul = payload
        .iter()
        .position(|&b| b == 0)
        .context("객체 헤더가 손상되었습니다")?;
    let header = std::str::from_utf8(&payload[..nul]).context("객체 헤더 인코딩 오류")?;
    let obj_type = header
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string();
    let body = payload[nul + 1..].to_vec();
    Ok((obj_type, body))
}

/// 파일 내용을 blob 으로 저장하고 해시를 반환한다.
pub fn write_blob(repo: &Repo, content: &[u8]) -> Result<String> {
    let mut blob = Blob::new(content.to_vec());
    let hash = blob.hash().to_string();
    write_object(repo, "blob", &hash, content)?;
    Ok(hash)
}

/// Tree 객체를 저장하고 해시를 반환한다.
///
/// 객체 id 는 core 의 Tree::hash 규칙을, 저장 body 는 엔트리 JSON 을 사용한다.
pub fn write_tree(repo: &Repo, tree: &mut Tree) -> Result<String> {
    let hash = tree.hash().to_string();
    let body = serde_json::to_vec(tree.entries()).context("tree 직렬화 실패")?;
    write_object(repo, "tree", &hash, &body)?;
    Ok(hash)
}

/// Commit 객체를 저장하고 해시를 반환한다.
pub fn write_commit(repo: &Repo, commit: &mut Commit) -> Result<String> {
    let hash = commit.hash().to_string();
    let body = serde_json::to_vec(commit).context("commit 직렬화 실패")?;
    write_object(repo, "commit", &hash, &body)?;
    Ok(hash)
}
