// ============================================
// 공통 에러 타입 정의
// ============================================
// thiserror 크레이트를 사용해 커스텀 에러 타입 정의
// 장점: 보일러플레이트 코드 줄임, Display/Error 트레이트 자동 구현

use thiserror::Error;  // Error derive 매크로 가져오기

// #[derive(Error, Debug)]: Error와 Debug 트레이트 자동 구현
// #[error("...")]: Display 트레이트 구현 (에러 메시지)
#[derive(Error, Debug)]
pub enum AppError {
    // 리소스를 찾을 수 없음
    // {0}은 첫 번째 필드를 의미 (String)
    #[error("Not found: {0}")]
    NotFound(String),

    // 잘못된 입력
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    // 내부 서버 에러
    #[error("Internal error: {0}")]
    Internal(String),

    // 인증 실패
    #[error("Unauthorized")]
    Unauthorized,

    // Git 관련 에러
    #[error("Git error: {0}")]
    Git(String),
}

// 사용 예시:
//
// fn find_repo(id: &str) -> Result<Repo, AppError> {
//     // 못 찾으면 에러 반환
//     Err(AppError::NotFound(format!("Repository {}", id)))
// }
//
// match result {
//     Ok(repo) => println!("Found: {}", repo.name),
//     Err(e) => println!("Error: {}", e),  // "Error: Not found: Repository xxx"
// }
