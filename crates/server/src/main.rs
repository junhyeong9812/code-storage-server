// =============================================================================
// CTS Server 진입점 (main.rs)
// =============================================================================
//
// 부트스트랩 순서:
// 1. .env 로드 (dotenvy)
// 2. 로깅 초기화 (tracing-subscriber, RUST_LOG 존중)
// 3. PostgreSQL 연결 풀 생성
// 4. 어댑터 → AppState 조립
// 5. axum 라우터(app) 빌드 후 서버 실행
//
// 실행: cargo run -p server   (바이너리 이름: cts-server)
//
// 파일 위치: crates/server/src/main.rs
// =============================================================================

use std::sync::Arc;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

use server::repository::infrastructure::adapters::{
    FileBlobStorage, PgObjectRepository, PgRepositoryRepository,
};
use server::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. .env 로드 (없어도 무시)
    dotenvy::dotenv().ok();

    // 2. 로깅 초기화
    //    RUST_LOG 가 있으면 그 설정, 없으면 info 레벨 기본값.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    // 3. DB 연결 풀
    let database_url =
        std::env::var("DATABASE_URL").context("환경변수 DATABASE_URL 이 필요합니다")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("PostgreSQL 연결에 실패했습니다")?;
    tracing::info!("PostgreSQL 연결 성공");

    // 4. 어댑터 → AppState
    let storage_path =
        std::env::var("STORAGE_PATH").unwrap_or_else(|_| "./storage".to_string());
    let repositories = Arc::new(PgRepositoryRepository::new(pool.clone()));
    let objects = Arc::new(PgObjectRepository::new(pool));
    let blobs = Arc::new(FileBlobStorage::new(storage_path));
    let state = AppState::new(repositories, objects, blobs);

    // 5. 라우터 빌드 + 서버 실행
    let app = server::app(state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("{host}:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("주소 바인딩 실패: {addr}"))?;
    tracing::info!("CTS Server 가 http://{addr} 에서 실행 중");

    axum::serve(listener, app)
        .await
        .context("서버 실행 중 오류")?;

    Ok(())
}
