// ============================================
// shared 크레이트 진입점 (lib.rs)
// ============================================
// lib.rs는 라이브러리 크레이트의 루트 파일
// 이 파일에서 모듈을 선언하면 외부에서 사용 가능
//
// 사용 예시 (다른 크레이트에서):
//   use shared::error::AppError;
//   use shared::types::new_id;

// pub mod: 공개 모듈 선언
// 이 선언은 "src/error/mod.rs 파일을 error라는 모듈로 공개한다"는 의미
pub mod error;   // 공통 에러 타입들
pub mod types;   // 공통 타입들 (Id, Timestamp)
