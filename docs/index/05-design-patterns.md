# 05. 디자인 패턴

[← 04 객체 모델](04-core-object-model.md) | [인덱스](README.md) | [다음: 06 워크플로우 →](06-workflows.md)

CTS가 의도적으로 적용한 패턴들. 각 패턴은 "무엇/왜/어디서" 순으로.

## 1. Ports & Adapters (Hexagonal)
- **무엇**: 도메인이 외부를 trait(포트)로만 알고, 구현(어댑터)은 밖에 둔다.
- **왜**: 도메인 로직을 DB/HTTP 없이 테스트·이해 가능. 구현 교체가 쉬움.
- **어디서**: `domain/ports/*.rs`(trait) ↔ `infrastructure/adapters/*.rs`(구현).
  예) `BlobStorage` ↔ `FileBlobStorage`, `TokenService` ↔ `JwtTokenService`.

## 2. 의존성 역전 + 주입 (DI)
- **무엇**: 상위(도메인)가 하위(인프라)를 직접 참조하지 않고 추상(trait)에 의존. 구현은 런타임에 주입.
- **어디서**: `AppState`가 `Arc<dyn Port>` 묶음을 보유, `main.rs`(합성 루트)가 어댑터를 생성·주입.

## 3. Repository 패턴
- **무엇**: 영속화를 컬렉션처럼 다루는 추상(`create/find_by_id/list/delete`).
- **어디서**: `RepositoryRepository`, `UserRepository`, `BuildRepository`, `ObjectRepository`, `CollaboratorRepository`.

## 4. Value Object + "parse, don't validate"
- **무엇**: 원시 문자열 대신 **검증된 값 타입**. 인스턴스가 존재한다 == 이미 유효하다.
- **왜**: 잘못된 값이 도메인 안으로 못 들어옴. 타입이 곧 불변식.
- **어디서**: `RepositoryName::parse`, `Email::parse`, `Username::parse`, `BuildStatus::from_db`, `Role`.

## 5. Newtype ID
- **무엇**: `RepositoryId(Uuid)`처럼 UUID를 감싼 식별자 타입.
- **왜**: `RepositoryId` 자리에 `UserId`를 실수로 넣으면 **컴파일 에러**. 의미가 타입에 박힘.
- **어디서**: `repository/domain/value_objects/ids.rs`(`define_id!` 매크로), `user`·`build`도 각자.

## 6. Aggregate Root
- **무엇**: 일관성 경계의 진입점 엔티티. 비공개 필드 + 게터로 불변식 보호.
- **어디서**: `Repository`, `User`, `Build` 엔티티(필드 private, `new`/`from_persistence` 생성자).

## 7. DTO (Data Transfer Object)
- **무엇**: API 경계 입출력 구조를 도메인 엔티티와 분리.
- **왜**: 내부 모델을 외부에 노출하지 않고, API 스키마를 독립적으로 진화.
- **어디서**: `application/dto/`(`CreateRepositoryRequest`, `RepositoryResponse`, `AuthResponse`, ...).

## 8. 유스케이스(Application Service)
- **무엇**: "하나의 비즈니스 동작"을 함수로. 포트만 의존, 기술은 모름.
- **어디서**: `application/use_cases/`(`create_repository`, `push`, `pull`, `register`, `login`, `run_build`, ...).

## 9. Axum Extractor (FromRequestParts)
- **무엇**: 요청에서 값을 꺼내는 합성 가능한 추출기. 인증을 핸들러 인자로 선언적으로 요구.
- **어디서**: `auth.rs`의 `AuthUser`(필수, 없으면 401), `MaybeAuthUser`(선택, 공개 읽기용).
  ```rust
  async fn create_handler(State(state), auth: AuthUser, Json(req)) { ... }
  // ↑ 핸들러 시그니처에 AuthUser를 넣는 것만으로 "인증 필수"가 강제됨
  ```

## 10. 내용 주소 지정 저장 (Content-addressed storage)
- 객체 이름=내용 해시. 중복 제거·무결성·간단한 동기화. ([04장](04-core-object-model.md))

## 11. 심볼릭 참조 (Symbolic ref)
- **무엇**: `HEAD`가 커밋이 아니라 브랜치를 가리킴(`ref: refs/heads/main`).
- **어디서**: CLI `.cts/HEAD`, `refs.rs`(current_branch/update_branch/set_head).

## 12. 권한 사다리 (AccessLevel)
- **무엇**: `None < Read < Write < Admin < Owner` 순서 비교로 인가 판정.
- **어디서**: `auth.rs`의 `effective_level` + `require_read/write/admin/owner`. ([07장](07-server-domains.md))

## 13. 토큰 철회 목록 (jti blacklist)
- **무엇**: 상태없는 JWT에 고유 `jti`를 넣고, 로그아웃 시 DB에 기록 → 검증 때 확인.
- **어디서**: `TokenRevocation` 포트 + `revoked_tokens` 테이블.

## 14. 와이어 프로토콜 공유 타입
- **무엇**: 서버·CLI가 같은 직렬화 타입을 쓰도록 `shared`에 둠.
- **어디서**: `shared::protocol`(`WireBlob/Tree/Commit`, `ObjectBundle`, `Push/PullRequest/Response`).

[← 04 객체 모델](04-core-object-model.md) | [인덱스](README.md) | [다음: 06 워크플로우 →](06-workflows.md)
