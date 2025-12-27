// ============================================
// Build 도메인 레이어
// ============================================

pub mod entities;       // Build, Pipeline, BuildStep 등
pub mod value_objects;  // BuildId, BuildStatus 등
pub mod ports;          // BuildQueuePort, DockerPort 등
pub mod services;       // 빌드 관련 도메인 서비스
