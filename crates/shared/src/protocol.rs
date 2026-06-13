// =============================================================================
// Push/Pull 와이어 프로토콜 (protocol.rs)
// =============================================================================
//
// CLI ↔ Server 사이에서 JSON 으로 주고받는 객체 번들/요청/응답 타입.
// 서버와 CLI 가 동일한 타입을 쓰도록 shared 크레이트에 둔다.
//
// 설계(의도적 단순화):
// - 아키텍처 §6 의 개별 객체 엔드포인트 대신, 커밋에서 도달 가능한 객체
//   묶음(closure)을 한 번에 전송하는 bulk push/pull 을 사용한다.
// - blob 내용은 base64 의존성 없이 Vec<u8>(JSON 숫자 배열)로 전송한다.
//   (학습용. 대용량/효율보다 단순함 우선)
//
// 파일 위치: crates/shared/src/protocol.rs
// =============================================================================

use serde::{Deserialize, Serialize};

/// 파일 내용 객체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireBlob {
    pub hash: String,
    /// 원본 파일 바이트
    pub content: Vec<u8>,
}

/// 트리 엔트리 (파일 또는 하위 디렉토리 참조)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireTreeEntry {
    pub name: String,
    /// 파일 모드 ("100644" / "100755" / "040000")
    pub mode: String,
    /// 참조 종류 ("blob" 또는 "tree")
    pub object_type: String,
    /// 참조 대상 객체 해시
    pub hash: String,
}

/// 디렉토리 구조 객체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireTree {
    pub hash: String,
    pub entries: Vec<WireTreeEntry>,
}

/// 커밋 객체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCommit {
    pub hash: String,
    pub tree_hash: String,
    pub parent_hash: Option<String>,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    /// RFC3339 타임스탬프
    pub timestamp: String,
}

/// 객체 번들
///
/// 의존성 순서로 채워 보낸다:
/// - blobs: 임의 순서
/// - trees: 리프 우선(자식 트리가 먼저)
/// - commits: 오래된 것 우선(부모가 먼저)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectBundle {
    pub blobs: Vec<WireBlob>,
    pub trees: Vec<WireTree>,
    pub commits: Vec<WireCommit>,
}

/// Push 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    /// 갱신할 브랜치 이름
    pub branch: String,
    /// 브랜치 head 가 될 커밋 해시
    pub commit_hash: String,
    pub objects: ObjectBundle,
}

/// Push 응답 (저장 결과 요약)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushResponse {
    pub branch: String,
    pub commit_hash: String,
    pub stored_blobs: usize,
    pub stored_trees: usize,
    pub stored_commits: usize,
}

/// Pull 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullResponse {
    pub branch: String,
    /// 브랜치 head 커밋 해시 (커밋이 없으면 None)
    pub commit_hash: Option<String>,
    pub objects: ObjectBundle,
}
