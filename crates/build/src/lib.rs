// ============================================
// build 크레이트 진입점 (lib.rs)
// ============================================
// Build 도메인 (Bounded Context)
// CI/CD 파이프라인 실행, 빌드 큐 관리 담당
//
// 주요 기능:
//   - 빌드 트리거 (push 이벤트 등)
//   - 빌드 큐 관리
//   - Docker 컨테이너에서 빌드 실행
//   - 빌드 로그 스트리밍
//   - 아티팩트 저장

pub mod domain;          // 도메인 레이어
pub mod application;     // 응용 레이어
pub mod infrastructure;  // 인프라 레이어
pub mod api;             // 표현 레이어
