# Cargo.toml 가이드

Rust 프로젝트의 설정 파일입니다. JavaScript의 `package.json`, Java의 `pom.xml`과 같은 역할을 합니다.

## 목차

1. [기본 구조](#1-기본-구조)
2. [워크스페이스](#2-워크스페이스)
3. [패키지 설정](#3-패키지-설정)
4. [의존성 관리](#4-의존성-관리)
5. [버전 표기법](#5-버전-표기법)
6. [Features](#6-features)
7. [자주 쓰는 명령어](#7-자주-쓰는-명령어)

---

## 1. 기본 구조

가장 단순한 `Cargo.toml`:

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2021"

[dependencies]
```

| 섹션                   | 설명                   |
| ---------------------- | ---------------------- |
| `[package]`            | 프로젝트 메타데이터    |
| `[dependencies]`       | 런타임 의존성          |
| `[dev-dependencies]`   | 개발/테스트용 의존성   |
| `[build-dependencies]` | 빌드 스크립트용 의존성 |

---

## 2. 워크스페이스

여러 크레이트(패키지)를 하나의 프로젝트로 관리할 때 사용합니다.

### 워크스페이스 루트 Cargo.toml

```toml
[workspace]
resolver = "2"

members = [
    "crates/repository",
    "crates/build",
    "crates/user",
    "crates/shared",
]
```

| 필드       | 설명                                                |
| ---------- | --------------------------------------------------- |
| `resolver` | 의존성 해석 버전. `"2"`가 최신 (2021 에디션 기본값) |
| `members`  | 워크스페이스에 포함할 크레이트 경로 목록            |

### 장점

- 모든 크레이트가 하나의 `Cargo.lock` 공유 (버전 일관성)
- 루트에서 `cargo build`하면 전체 빌드
- 공통 의존성 버전을 한 곳에서 관리 가능

---

## 3. 패키지 설정

### [package] 섹션

```toml
[package]
name = "my-crate"           # 크레이트 이름 (필수)
version = "0.1.0"           # 버전 (필수)
edition = "2021"            # Rust 에디션 (필수)
authors = ["Name <email>"]  # 작성자
license = "MIT"             # 라이선스
description = "설명"         # 짧은 설명
repository = "https://..."  # Git 저장소 URL
readme = "README.md"        # README 파일 경로
keywords = ["key1", "key2"] # 검색 키워드 (최대 5개)
categories = ["..."]        # crates.io 카테고리
```

### Rust 에디션 (Edition)

| 에디션 | 출시년도 | 특징                               |
| ------ | -------- | ---------------------------------- |
| 2015   | 2015     | Rust 1.0 최초                      |
| 2018   | 2018     | 모듈 시스템 개선, async/await 준비 |
| 2021   | 2021     | 현재 권장. 클로저 캡처 개선 등     |

에디션은 하위 호환됩니다. 2021 에디션 코드가 2018 라이브러리 사용 가능.

### 워크스페이스 값 상속

워크스페이스 루트에서 공통 값 정의:

```toml
# 루트 Cargo.toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
```

각 크레이트에서 상속:

```toml
# crates/shared/Cargo.toml
[package]
name = "shared"
version.workspace = true      # 워크스페이스 값 사용
edition.workspace = true
license.workspace = true
```

---

## 4. 의존성 관리

### 기본 문법

```toml
[dependencies]
# 방법 1: 버전만 (crates.io에서 다운로드)
serde = "1.0"

# 방법 2: 상세 설정
tokio = { version = "1", features = ["full"] }

# 방법 3: 로컬 경로 (같은 워크스페이스 내 크레이트)
shared = { path = "../shared" }

# 방법 4: Git 저장소
some_crate = { git = "https://github.com/user/repo" }

# 방법 5: Git + 특정 브랜치/태그
some_crate = { git = "https://...", branch = "main" }
some_crate = { git = "https://...", tag = "v1.0.0" }
some_crate = { git = "https://...", rev = "abc123" }
```

### 워크스페이스 의존성 공유

루트에서 정의:

```toml
# 루트 Cargo.toml
[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
axum = "0.7"
```

각 크레이트에서 사용:

```toml
# crates/repository/Cargo.toml
[dependencies]
tokio.workspace = true    # 버전 명시 불필요
serde.workspace = true
axum.workspace = true
```

### 의존성 종류

```toml
[dependencies]
# 런타임에 필요한 라이브러리
serde = "1"

[dev-dependencies]
# 테스트/벤치마크에만 필요
mockall = "0.11"

[build-dependencies]
# build.rs 스크립트에서 사용
cc = "1.0"
```

### Optional 의존성

```toml
[dependencies]
serde_json = { version = "1", optional = true }

[features]
json = ["serde_json"]  # json feature 활성화 시 serde_json 포함
```

---

## 5. 버전 표기법

Cargo는 Semantic Versioning (SemVer)을 사용합니다.

```
MAJOR.MINOR.PATCH
  │     │     │
  │     │     └─ 버그 수정 (하위 호환)
  │     └─────── 기능 추가 (하위 호환)
  └───────────── 큰 변경 (하위 호환 안됨)
```

### 버전 범위 지정

| 표기            | 의미          | 허용 범위       |
| --------------- | ------------- | --------------- |
| `"1.2.3"`       | 캐럿 (기본값) | >=1.2.3, <2.0.0 |
| `"^1.2.3"`      | 캐럿 (명시적) | >=1.2.3, <2.0.0 |
| `"~1.2.3"`      | 틸드          | >=1.2.3, <1.3.0 |
| `"1.2.*"`       | 와일드카드    | >=1.2.0, <1.3.0 |
| `">=1.2.3"`     | 이상          | >=1.2.3         |
| `"<2.0.0"`      | 미만          | <2.0.0          |
| `">=1.2, <1.5"` | 범위          | >=1.2.0, <1.5.0 |
| `"=1.2.3"`      | 정확히        | 1.2.3만         |

### 실무 팁

```toml
# 일반적으로 이렇게 씀 (MAJOR만 지정)
tokio = "1"      # 1.x.x 아무거나 OK

# 특정 기능이 필요하면 MINOR까지
axum = "0.7"     # 0.7.x

# 버그 있는 버전 피할 때
serde = ">=1.0.180"
```

---

## 6. Features

라이브러리의 선택적 기능입니다. 필요한 것만 켜서 컴파일 시간과 바이너리 크기를 줄입니다.

### 사용법

```toml
[dependencies]
# 기본 features만
tokio = "1"

# 특정 features 활성화
tokio = { version = "1", features = ["rt", "net", "io-util"] }

# 모든 features
tokio = { version = "1", features = ["full"] }

# 기본 features 끄기
serde = { version = "1", default-features = false }

# 기본 끄고 특정 것만
serde = { version = "1", default-features = false, features = ["derive"] }
```

### 자주 쓰는 라이브러리 features

**tokio** (비동기 런타임):

```toml
tokio = { version = "1", features = ["full"] }
# 또는 필요한 것만
tokio = { version = "1", features = ["rt-multi-thread", "net", "io-util", "macros"] }
```

| feature           | 설명                 |
| ----------------- | -------------------- |
| `rt`              | 런타임               |
| `rt-multi-thread` | 멀티스레드 런타임    |
| `net`             | TCP/UDP              |
| `io-util`         | AsyncRead/AsyncWrite |
| `macros`          | #[tokio::main] 등    |
| `full`            | 전부 다              |

**serde** (직렬화):

```toml
serde = { version = "1", features = ["derive"] }
```

| feature  | 설명                                     |
| -------- | ---------------------------------------- |
| `derive` | #[derive(Serialize, Deserialize)] 매크로 |
| `std`    | 표준 라이브러리 (기본값)                 |

**sqlx** (데이터베이스):

```toml
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] }
```

| feature         | 설명              |
| --------------- | ----------------- |
| `runtime-tokio` | tokio 런타임 사용 |
| `sqlite`        | SQLite 지원       |
| `postgres`      | PostgreSQL 지원   |
| `mysql`         | MySQL 지원        |

### 내 크레이트에 feature 정의하기

```toml
[features]
default = ["json"]           # 기본 활성화
json = ["serde_json"]        # json feature -> serde_json 의존성
full = ["json", "xml"]       # 여러 feature 묶음

[dependencies]
serde_json = { version = "1", optional = true }
```

---

## 7. 자주 쓰는 명령어

### 프로젝트 관리

```bash
cargo new my-project      # 새 프로젝트 (바이너리)
cargo new my-lib --lib    # 새 프로젝트 (라이브러리)
cargo init                # 현재 폴더를 프로젝트로
```

### 빌드 & 실행

```bash
cargo build               # 디버그 빌드
cargo build --release     # 릴리즈 빌드 (최적화)
cargo run                 # 빌드 + 실행
cargo run --release       # 릴리즈로 실행
```

### 검사 & 테스트

```bash
cargo check               # 컴파일 검사만 (빠름)
cargo test                # 테스트 실행
cargo clippy              # 린트 (권장사항 검사)
cargo fmt                 # 코드 포맷팅
```

### 의존성 관리

```bash
cargo add tokio           # 의존성 추가 (Cargo.toml 자동 수정)
cargo add tokio -F full   # features와 함께 추가
cargo remove tokio        # 의존성 제거
cargo update              # Cargo.lock 업데이트
cargo tree                # 의존성 트리 보기
```

### 워크스페이스

```bash
cargo build                        # 전체 빌드
cargo build -p shared              # 특정 크레이트만 빌드
cargo test -p repository           # 특정 크레이트 테스트
cargo run -p api                   # 특정 크레이트 실행
```

### 정보 확인

```bash
cargo --version           # Cargo 버전
cargo doc --open          # 문서 생성 후 브라우저로 열기
cargo search serde        # crates.io 검색
```

---

## 이 프로젝트의 Cargo.toml 구조

```
code-storage-server/
├── Cargo.toml              # 워크스페이스 루트
└── crates/
    ├── shared/
    │   └── Cargo.toml      # 공유 커널
    ├── repository/
    │   └── Cargo.toml      # Repository 도메인
    ├── build/
    │   └── Cargo.toml      # Build 도메인
    └── user/
        └── Cargo.toml      # User 도메인
```

의존성 흐름:

```
repository ─┐
build ──────┼──▶ shared
user ───────┘
```

---

## 참고 자료

- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Cargo.toml 명세](https://doc.rust-lang.org/cargo/reference/manifest.html)
- [crates.io](https://crates.io/) - Rust 패키지 저장소
