# OVERVIEW: Phase 8 — 인증/인가 (User)

> 목적: 이 구현의 **추상 진입점**. Phase 8 은 "시드 유저 고정(SEEDED_OWNER_ID)" → "실제 JWT 인증 + 공개읽기/소유자쓰기 인가"로 전환한 단계다. 무엇을·어떤 순서/분기로 도는지 한눈에 본 뒤 TECHNICAL/changelog/learned 로 내려간다.
> 4문서 경계: OVERVIEW = 무엇/순서/분기 · TECHNICAL = 왜/어떻게 · changelog = 이번 diff 선택과 이유(J/M/G) · learned = 요소 카탈로그.

## 주요 포인트 (30초 지도)

- **회원가입/로그인이 곧 토큰 발급기다** — `register`/`login` 유스케이스가 끝에서 항상 `tokens.issue(...)` 를 호출해 `AuthResponse{token,user}` 를 반환한다. 핵심 메커니즘 키워드: bcrypt 해시 저장·검증, HS256 JWT 발급. → `TECHNICAL §개념 2/3`, 발급 근거 `changelog J-7`.
- **인증은 axum extractor 두 개로 들어온다** — `AuthUser`(Bearer 필수, 없으면 401)와 `MaybeAuthUser`(옵션, 잘못돼도 None). 핸들러 인자 자리에 타입만 적으면 `FromRequestParts` 가 토큰을 검증한다. 까다로운 곳: `MaybeAuthUser` 의 `Rejection = Infallible` 라 "토큰 오류 = 인증 안 됨"으로 흡수된다. → `TECHNICAL §개념 1·동작 방식`, `changelog J-5`.
- **인가는 두 헬퍼로 수렴한다** — 쓰기 경로는 `require_owner`(소유자 아니면 403), 읽기 경로는 `require_read`(비공개는 소유자만, 아니면 **404 은닉**). repository/build 핸들러가 공용으로 호출한다. 위험 키워드: 403 vs 404 의도적 구분, 목록 가시성 필터. → `TECHNICAL §불변조건·실패 모드`, `changelog J-8`.
- **에러 한 칸이 늘었다** — `shared::AppError::Forbidden(String)` 추가 → 서버 `ApiError::into_response` 가 403 으로 매핑. 401(Unauthorized)/403(Forbidden)/404(NotFound) 가 보안 경계의 세 출구다. → `changelog J-1`.
- **CLI 가 토큰을 서버 URL별로 들고 다닌다** — `cts register`/`cts login` 이 토큰을 `~/.config/cts/credentials.json` 에 저장하고, push/remote 는 토큰 필수, pull/clone 은 토큰 옵션으로 `Authorization: Bearer` 를 붙인다. → `changelog J-9/J-10`, `learned §5`.
- **Email 검증 버그를 이 Phase 안에서 고쳤다(c7cd12d)** — local-part 내부 공백("a b@c.com")이 통과하던 구멍을 `!trimmed.contains(char::is_whitespace)` 한 줄로 막았다. 왜 비직관인지: `learned §4`·`TECHNICAL §함정`·`changelog J-12`.

## 워크플로우 (절차 + 분기)

### A. 자격 취득 — register / login → 토큰 발급

```
cts register/login (CLI)            POST /api/auth/{register|login}
  │  비밀번호 숨김 입력                    │
  │  (rpassword / CTS_PASSWORD)           ▼
  └──── HTTP ───▶ [handler] ──▶ [use_case]
                                  │
              register:           │  login:
              Username/Email parse│  find_by_username
              password.len()>=6   │      │ 없음 ─▶ 401(Unauthorized)
              exists_username? ───┤      ▼
                 예 ─▶ 409         │  hasher.verify(pw, hash)
              exists_email? ──────┤      │ 불일치 ─▶ 401(동일 에러)
                 예 ─▶ 409         │      ▼
              hasher.hash(pw)      │  tokens.issue(id, name)
              users.create        │
              tokens.issue ───────┴──▶ AuthResponse{token, user}
                                          │
                       ◀── 201/200 + JSON ┘
  CLI: credentials.set(server_url, {token, username}) → credentials.json
```

### B. 인증된 요청 — AuthUser 추출 → 인가 분기

```
요청 (Authorization: Bearer <jwt>?)
  │
  ▼
[extractor]  ── 핸들러 인자 타입에 따라 ──┐
  │                                       │
  ├─ AuthUser (쓰기 핸들러)               ├─ MaybeAuthUser (읽기 핸들러)
  │   bearer(parts)?                      │   AuthUser::from_request_parts(..).ok()
  │     없음 ─▶ 401                        │     실패 ─▶ None (절대 거부 안 함)
  │   tokens.verify(jwt)                  │
  │     실패 ─▶ 401                        │
  ▼                                       ▼
[require_owner(state,id,auth)]      [require_read(state,id,maybe)]
  load_repository(id)                 load_repository(id)
    없음 ─▶ 404                          없음 ─▶ 404
  owner_id == auth.user_id?           is_private?
    아니오 ─▶ 403(Forbidden)             ├─ 공개 ─▶ 통과
    예 ─▶ 통과                           └─ 비공개 & !소유자 ─▶ 404(은닉)
  ▼                                       ▼
[use_case 실행] ─▶ 응답              [use_case 실행] ─▶ 응답
```

> 박스가 "왜 그렇게 동작하는가"(예: 401 vs 403 vs 404 의 의미론, Infallible 선택, 동일 401 로 사용자/비번 구분 은닉)는 TECHNICAL 로 보낸다.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| JWT/bcrypt/extractor 가 왜 그렇게 동작하나, 불변조건·실패모드 | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거, 파일별) | changelog (J-1 ~ J-12, M, G) |
| 어떤 라이브러리/함수/패턴을 어떻게 썼나 (bcrypt·jsonwebtoken·rpassword·axum extractor) | learned |
| Email 버그 수정 전/후 | learned §4 · TECHNICAL §함정 · changelog J-12 |
