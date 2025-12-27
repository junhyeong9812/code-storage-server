// ============================================
// user 크레이트 진입점 (lib.rs)
// ============================================
// User 도메인 (Bounded Context)
// 사용자 인증, 권한 관리 담당
//
// 주요 기능:
//   - 회원가입 / 로그인
//   - JWT 토큰 발급/검증
//   - 권한 관리 (저장소별 접근 권한)

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod api;
