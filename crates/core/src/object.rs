// =============================================================================
// 객체 모델 (object.rs)
// =============================================================================
//
// CTS의 핵심 데이터 구조 정의
// Git과 유사하지만 독립적인 포맷
//
// 객체 타입:
// - Blob: 파일 내용 (바이너리/텍스트)
// - Tree: 디렉토리 구조 (파일/폴더 목록)
// - Commit: 스냅샷 (tree + 메타데이터)
//
// 파일 위치: crates/core/src/object.rs
//
// Git과의 차이점:
// - SHA-256 사용 (Git은 SHA-1)
// - JSON 직렬화 지원
// - 타입 안전한 Rust 구조체
// =============================================================================

use serde::{Deserialize, Serialize};
use crate::hash::{Hasher, HASH_HEX_LENGTH};

// =============================================================================
// 객체 타입 열거형
// =============================================================================

/// CTS 객체 타입
///
/// 모든 CTS 객체는 이 세 가지 타입 중 하나
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObjectType {
    /// 파일 내용
    Blob,
    /// 디렉토리 구조
    Tree,
    /// 커밋 (스냅샷)
    Commit,
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectType::Blob => write!(f, "blob"),
            ObjectType::Tree => write!(f, "tree"),
            ObjectType::Commit => write!(f, "commit"),
        }
    }
}

// =============================================================================
// Blob (파일 내용)
// =============================================================================

/// Blob - 파일 내용 저장
///
/// 파일의 실제 바이너리 내용을 저장
/// 파일 이름, 경로, 권한 등은 저장하지 않음 (Tree에서 관리)
///
/// # 특징
/// - 내용이 같으면 해시도 같음 → 중복 제거 가능
/// - 압축 저장으로 용량 절약
///
/// # Example
/// ```
/// use core::object::Blob;
///
/// let content = b"hello world";
/// let blob = Blob::new(content.to_vec());
///
/// println!("Hash: {}", blob.hash());
/// println!("Size: {} bytes", blob.size());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blob {
    /// 파일 내용 (바이너리)
    content: Vec<u8>,
    /// 컨텐츠 해시 (SHA-256)
    /// 지연 계산(lazy) 또는 미리 계산
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

impl Blob {
    /// 새 Blob 생성
    ///
    /// # Arguments
    /// * `content` - 파일 내용
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            hash: None,  // 나중에 필요할 때 계산
        }
    }

    /// 해시와 함께 Blob 생성
    ///
    /// DB에서 로드할 때 사용 (이미 해시를 알고 있음)
    pub fn with_hash(content: Vec<u8>, hash: String) -> Self {
        Self {
            content,
            hash: Some(hash),
        }
    }

    /// 파일 내용 반환
    pub fn content(&self) -> &[u8] {
        &self.content
    }

    /// 파일 크기 (바이트)
    pub fn size(&self) -> usize {
        self.content.len()
    }

    /// 해시 계산 및 반환
    ///
    /// 처음 호출 시 계산, 이후 캐시된 값 반환
    pub fn hash(&mut self) -> &str {
        if self.hash.is_none() {
            let hasher = Hasher::new();
            // Blob 해시: "blob {size}\0{content}" 형식 (Git 호환)
            let header = format!("blob {}\0", self.content.len());
            let mut data = header.into_bytes();
            data.extend_from_slice(&self.content);
            self.hash = Some(hasher.hash_bytes(&data));
        }
        self.hash.as_ref().unwrap()
    }

    /// 해시 반환 (불변 참조, 이미 계산된 경우만)
    pub fn cached_hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }

    /// 텍스트 파일인지 확인 (휴리스틱)
    ///
    /// NUL 바이트가 없으면 텍스트로 추정
    pub fn is_text(&self) -> bool {
        !self.content.contains(&0)
    }

    /// UTF-8 텍스트로 변환 시도
    pub fn as_text(&self) -> Option<&str> {
        std::str::from_utf8(&self.content).ok()
    }
}

// =============================================================================
// Tree (디렉토리 구조)
// =============================================================================

/// TreeEntry - 트리의 개별 항목
///
/// 파일 또는 하위 디렉토리를 나타냄
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// 파일/디렉토리 이름 (경로 아님, 이름만)
    pub name: String,
    /// 객체 타입 (blob 또는 tree)
    pub object_type: ObjectType,
    /// 참조하는 객체의 해시
    pub hash: String,
    /// 파일 모드 (예: "100644" = 일반 파일, "100755" = 실행 파일, "040000" = 디렉토리)
    pub mode: String,
}

impl TreeEntry {
    /// 새 파일 엔트리 생성
    pub fn file(name: String, hash: String) -> Self {
        Self {
            name,
            object_type: ObjectType::Blob,
            hash,
            mode: "100644".to_string(),  // 일반 파일
        }
    }

    /// 새 실행 파일 엔트리 생성
    pub fn executable(name: String, hash: String) -> Self {
        Self {
            name,
            object_type: ObjectType::Blob,
            hash,
            mode: "100755".to_string(),  // 실행 파일
        }
    }

    /// 새 디렉토리 엔트리 생성
    pub fn directory(name: String, hash: String) -> Self {
        Self {
            name,
            object_type: ObjectType::Tree,
            hash,
            mode: "040000".to_string(),  // 디렉토리
        }
    }

    /// 파일인지 확인
    pub fn is_file(&self) -> bool {
        self.object_type == ObjectType::Blob
    }

    /// 디렉토리인지 확인
    pub fn is_directory(&self) -> bool {
        self.object_type == ObjectType::Tree
    }
}

/// Tree - 디렉토리 구조
///
/// 디렉토리 안의 파일/폴더 목록
/// 각 항목은 이름 + 타입 + 해시로 구성
///
/// # Example
/// ```
/// use core::object::{Tree, TreeEntry};
///
/// let mut tree = Tree::new();
/// tree.add_entry(TreeEntry::file("README.md".into(), "abc123...".into()));
/// tree.add_entry(TreeEntry::directory("src".into(), "def456...".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    /// 트리 항목들 (이름순 정렬 유지)
    entries: Vec<TreeEntry>,
    /// 트리 해시 (캐시)
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

impl Tree {
    /// 빈 트리 생성
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            hash: None,
        }
    }

    /// 항목들로 트리 생성
    pub fn with_entries(mut entries: Vec<TreeEntry>) -> Self {
        // 이름순 정렬 (Git 호환)
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            entries,
            hash: None,
        }
    }

    /// 항목 추가
    ///
    /// 추가 후 자동 정렬됨
    pub fn add_entry(&mut self, entry: TreeEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
        self.hash = None;  // 해시 무효화
    }

    /// 모든 항목 반환
    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }

    /// 이름으로 항목 찾기
    pub fn find(&self, name: &str) -> Option<&TreeEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// 항목 개수
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 해시 계산
    pub fn hash(&mut self) -> &str {
        if self.hash.is_none() {
            let hasher = Hasher::new();
            // Tree 해시: 모든 엔트리의 정렬된 직렬화
            let mut data = Vec::new();
            for entry in &self.entries {
                // "{mode} {name}\0{hash_bytes}" 형식 (Git 유사)
                let line = format!("{} {}\0", entry.mode, entry.name);
                data.extend_from_slice(line.as_bytes());
                // 해시는 hex가 아닌 raw bytes로 (간단히 hex 사용)
                data.extend_from_slice(entry.hash.as_bytes());
            }
            let header = format!("tree {}\0", data.len());
            let mut full_data = header.into_bytes();
            full_data.extend(data);
            self.hash = Some(hasher.hash_bytes(&full_data));
        }
        self.hash.as_ref().unwrap()
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Commit (커밋)
// =============================================================================

/// Commit - 스냅샷
///
/// 특정 시점의 전체 파일 상태를 나타냄
/// Tree를 참조하고, 메타데이터(작성자, 메시지 등) 포함
///
/// # 구조
/// ```text
/// Commit
///   ├── tree_hash ──→ Tree (루트 디렉토리)
///   ├── parent_hash ──→ 이전 Commit (없으면 첫 커밋)
///   ├── message: "커밋 메시지"
///   ├── author: "이름 <이메일>"
///   └── timestamp: "2024-01-15T10:30:00Z"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    /// 루트 트리 해시
    pub tree_hash: String,
    /// 부모 커밋 해시 (첫 커밋이면 None)
    pub parent_hash: Option<String>,
    /// 커밋 메시지
    pub message: String,
    /// 작성자 이름
    pub author_name: String,
    /// 작성자 이메일
    pub author_email: String,
    /// 커밋 시간 (ISO 8601 형식)
    pub timestamp: String,
    /// 커밋 해시 (캐시)
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
}

impl Commit {
    /// 새 커밋 생성
    pub fn new(
        tree_hash: String,
        parent_hash: Option<String>,
        message: String,
        author_name: String,
        author_email: String,
        timestamp: String,
    ) -> Self {
        Self {
            tree_hash,
            parent_hash,
            message,
            author_name,
            author_email,
            timestamp,
            hash: None,
        }
    }

    /// 첫 커밋 생성 (부모 없음)
    pub fn initial(
        tree_hash: String,
        message: String,
        author_name: String,
        author_email: String,
        timestamp: String,
    ) -> Self {
        Self::new(tree_hash, None, message, author_name, author_email, timestamp)
    }

    /// 첫 커밋인지 확인
    pub fn is_initial(&self) -> bool {
        self.parent_hash.is_none()
    }

    /// 해시 계산
    pub fn hash(&mut self) -> &str {
        if self.hash.is_none() {
            let hasher = Hasher::new();
            // Commit 해시: 메타데이터 직렬화
            let parent = self.parent_hash.as_deref().unwrap_or("");
            let content = format!(
                "tree {}\nparent {}\nauthor {} <{}>\ndate {}\n\n{}",
                self.tree_hash,
                parent,
                self.author_name,
                self.author_email,
                self.timestamp,
                self.message
            );
            let header = format!("commit {}\0", content.len());
            let full_data = format!("{}{}", header, content);
            self.hash = Some(hasher.hash_bytes(full_data.as_bytes()));
        }
        self.hash.as_ref().unwrap()
    }

    /// 캐시된 해시 반환
    pub fn cached_hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }
}

// =============================================================================
// 테스트
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blob_new() {
        let content = b"hello world";
        let blob = Blob::new(content.to_vec());

        assert_eq!(blob.content(), content);
        assert_eq!(blob.size(), 11);
        assert!(blob.is_text());
    }

    #[test]
    fn test_blob_hash() {
        let mut blob = Blob::new(b"hello world".to_vec());
        let hash = blob.hash();

        assert_eq!(hash.len(), HASH_HEX_LENGTH);

        // 같은 내용은 같은 해시
        let mut blob2 = Blob::new(b"hello world".to_vec());
        assert_eq!(blob.hash(), blob2.hash());
    }

    #[test]
    fn test_blob_binary() {
        let content = vec![0, 1, 2, 255];
        let blob = Blob::new(content);

        assert!(!blob.is_text());
        assert!(blob.as_text().is_none());
    }

    #[test]
    fn test_tree_entry() {
        let file = TreeEntry::file("README.md".into(), "abc123".into());
        assert!(file.is_file());
        assert!(!file.is_directory());
        assert_eq!(file.mode, "100644");

        let dir = TreeEntry::directory("src".into(), "def456".into());
        assert!(!dir.is_file());
        assert!(dir.is_directory());
        assert_eq!(dir.mode, "040000");
    }

    #[test]
    fn test_tree_sorted() {
        let mut tree = Tree::new();
        tree.add_entry(TreeEntry::file("z.txt".into(), "hash1".into()));
        tree.add_entry(TreeEntry::file("a.txt".into(), "hash2".into()));
        tree.add_entry(TreeEntry::file("m.txt".into(), "hash3".into()));

        let entries = tree.entries();
        assert_eq!(entries[0].name, "a.txt");
        assert_eq!(entries[1].name, "m.txt");
        assert_eq!(entries[2].name, "z.txt");
    }

    #[test]
    fn test_tree_hash() {
        let mut tree1 = Tree::with_entries(vec![
            TreeEntry::file("a.txt".into(), "hash1".into()),
            TreeEntry::file("b.txt".into(), "hash2".into()),
        ]);

        let mut tree2 = Tree::with_entries(vec![
            TreeEntry::file("b.txt".into(), "hash2".into()),
            TreeEntry::file("a.txt".into(), "hash1".into()),
        ]);

        // 순서가 달라도 정렬되어 같은 해시
        assert_eq!(tree1.hash(), tree2.hash());
    }

    #[test]
    fn test_commit() {
        let mut commit = Commit::initial(
            "tree_hash_123".into(),
            "Initial commit".into(),
            "John Doe".into(),
            "john@example.com".into(),
            "2024-01-15T10:30:00Z".into(),
        );

        assert!(commit.is_initial());
        assert_eq!(commit.hash().len(), HASH_HEX_LENGTH);
    }

    #[test]
    fn test_commit_with_parent() {
        let commit = Commit::new(
            "tree_hash_456".into(),
            Some("parent_hash_123".into()),
            "Second commit".into(),
            "Jane Doe".into(),
            "jane@example.com".into(),
            "2024-01-15T11:00:00Z".into(),
        );

        assert!(!commit.is_initial());
        assert_eq!(commit.parent_hash, Some("parent_hash_123".into()));
    }

    #[test]
    fn test_object_type_display() {
        assert_eq!(format!("{}", ObjectType::Blob), "blob");
        assert_eq!(format!("{}", ObjectType::Tree), "tree");
        assert_eq!(format!("{}", ObjectType::Commit), "commit");
    }
}