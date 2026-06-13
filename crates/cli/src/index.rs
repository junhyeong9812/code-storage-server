// =============================================================================
// 스테이징 영역 (index.rs)
// =============================================================================
//
// `.cts/index` (JSON) 는 다음 커밋에 포함될 파일 목록(스테이징)이다.
// 각 엔트리는 작업 트리의 파일 경로 → blob 해시 매핑.
//
// 파일 위치: crates/cli/src/index.rs
// =============================================================================

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repo::Repo;

/// 인덱스 엔트리 (스테이징된 파일 하나)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// 저장소 루트 기준 상대 경로 (슬래시 구분)
    pub path: String,
    /// 파일 내용의 blob 해시
    pub hash: String,
    /// 파일 모드 ("100644" 일반 / "100755" 실행)
    pub mode: String,
    /// 파일 크기 (바이트)
    pub size: u64,
}

/// 스테이징 영역
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Index {
    pub entries: Vec<IndexEntry>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    /// `.cts/index` 로드 (없으면 빈 인덱스)
    pub fn load(repo: &Repo) -> Result<Self> {
        let path = repo.index_path();
        if !path.exists() {
            return Ok(Self::new());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("인덱스를 읽을 수 없습니다: {}", path.display()))?;
        let index: Index = serde_json::from_str(&text).context("인덱스 파싱 실패")?;
        Ok(index)
    }

    /// `.cts/index` 저장 (경로순 정렬)
    pub fn save(&self, repo: &Repo) -> Result<()> {
        let mut sorted = self.clone();
        sorted.entries.sort_by(|a, b| a.path.cmp(&b.path));
        let text = serde_json::to_string_pretty(&sorted)?;
        std::fs::write(repo.index_path(), text)?;
        Ok(())
    }

    /// 엔트리 추가/갱신 (같은 경로가 있으면 교체)
    pub fn upsert(&mut self, entry: IndexEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == entry.path) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// 경로로 엔트리 조회
    pub fn get(&self, path: &str) -> Option<&IndexEntry> {
        self.entries.iter().find(|e| e.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, hash: &str) -> IndexEntry {
        IndexEntry {
            path: path.to_string(),
            hash: hash.to_string(),
            mode: "100644".to_string(),
            size: 1,
        }
    }

    #[test]
    fn upsert_adds_then_replaces() {
        let mut index = Index::new();
        index.upsert(entry("a.txt", "h1"));
        index.upsert(entry("b.txt", "h2"));
        assert_eq!(index.entries.len(), 2);

        // 같은 경로 재추가 → 교체 (개수 불변, 해시 갱신)
        index.upsert(entry("a.txt", "h1-new"));
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.get("a.txt").unwrap().hash, "h1-new");
    }

    #[test]
    fn get_missing_returns_none() {
        let index = Index::new();
        assert!(index.get("nope").is_none());
    }
}
