# Code Storage Server

> 코드 저장소 서버 - 개인용 Git 호스팅 + CI/CD 자동화 플랫폼

## 개요

Code Storage Server는 개인용 Git 저장소 호스팅과 CI/CD 자동화를 제공하는 플랫폼입니다.

Gitea, GitLab과 같은 서비스를 직접 구현하며 Rust를 학습하고, 나아가 서버리스 아키텍처까지 확장하는 것을 목표로 합니다.

## 주요 기능 (예정)

- **Git 저장소 관리** - 저장소 생성, 푸시/풀, 브랜치 관리
- **코드 브라우징** - 웹에서 코드 보기, diff 뷰어
- **CI/CD 자동화** - 푸시 시 자동 빌드, Docker 기반 격리 실행
- **빌드 로그** - 실시간 빌드 로그 스트리밍

## 기술 스택

- **Language**: Rust
- **Architecture**: DDD + Hexagonal + Layered
- **Web Framework**: Axum
- **Git**: git2-rs
- **Database**: SQLite → PostgreSQL

## 프로젝트 구조

```
crates/
├── repository/    # Git 저장소 도메인
├── build/         # CI/CD 빌드 도메인
├── user/          # 사용자 인증 도메인
└── shared/        # 공유 커널
```

## 시작하기

(추후 작성)

## 라이선스

MIT
