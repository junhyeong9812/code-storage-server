# OVERVIEW: Phase 10 — 토큰 철회 / 로그아웃 (jti 블랙리스트)

> 목적: 이 구현의 추상 진입점. Phase 8 의 stateless JWT 인증 위에 "로그아웃 = 토큰 철회" 를 어떻게 얹었는지를 한눈에 보고, 거기서 딥다이브로 내려간다.
> 범위: Phase 10 종료 스냅샷(`/tmp/cts-snapshots/phase10`). 커밋 c58c8b7(철회 인프라) → d5027d2(로그아웃 API) → 4c6a667(CLI logout) → d7b070b(docs).

## 주요 포인트 (5)

이 구현을 처음 여는 사람이 30초에 잡아야 하는 것. 메커니즘 이름·위험 키워드만 쓰고 동작 원리는 TECHNICAL 로 보낸다.

- **JWT 에 `jti`(랜덤 UUID) 클레임을 심어 토큰마다 고유 식별자를 부여한다** — stateless JWT 를 "지목해서" 철회할 수 있게 만드는 전제. 발급은 `JwtTokenService::issue`, 클레임 구조는 `Claims`. → 동작 원리 `TECHNICAL §개념1·동작 방식`
- **철회 목록은 DB 테이블 `revoked_tokens`(jti PRIMARY KEY)에 저장한다** — 메모리가 아닌 영속 스토어라 서버 재시작·다중 인스턴스에도 철회가 유지된다. 포트 `TokenRevocation`, 어댑터 `PgTokenRevocation`. → 외부 경계 `TECHNICAL §외부 경계`, 선택 이유 `changelog J-1`
- **인증 요청마다 `AuthUser` 추출기가 서명·만료 검증 후 `is_revoked(jti)` 를 한 번 더 조회한다** — 철회됐으면 401. 검증 비용에 DB 조회 1회가 추가되는 지점. → 실패 모드 `TECHNICAL §실패 모드`, 분기 아래 워크플로우
- **로그아웃 = `POST /api/auth/logout`(인증 필요) 이 현재 토큰의 jti 를 `expires_at`(토큰 exp)과 함께 철회 등록**하고 `204 No Content` 반환. → 계약 `changelog J-7`
- **TTL/정리 메커니즘은 "데이터만 있고 동작은 미구현"** — `expires_at` 컬럼은 만료 행 정리용으로 저장하지만 자동 삭제(cron)는 이번 범위 밖. 토큰 자체 수명은 30일 고정. → 함정 `TECHNICAL §함정`, 한계 `learned §9`

## 워크플로우 (절차 + 분기)

```
[로그아웃 경로]
(cts logout <server>)
  │
  ▼
[전역 자격증명 로드] ── 해당 서버 토큰 있음? ──┬─ 예 ─▶ [POST /api/auth/logout (Bearer)]
                                              │           │
                                              │           ├─ 서버 실패 ─▶ (무시: let _ =)
                                              │           └─ 성공/실패 무관 ─┐
                                              └─ 아니오 ──────────────────────┤
                                                                              ▼
                                                              [credentials.remove(server) + save]
                                                                              │
                                                                              ▼
                                                                   (로컬 자격증명 제거 완료)

[서버 logout_handler 내부]
POST /api/auth/logout
  │
  ▼
[AuthUser 추출 (인증)] ── 토큰 유효 & 미철회? ──┬─ 아니오 ─▶ (401 Unauthorized)
                                               └─ 예 ─▶ [logout use_case]
                                                          │  exp(i64) → DateTime(expires_at)
                                                          ▼
                                                  [revocation.revoke(jti, expires_at)]
                                                          │  INSERT ... ON CONFLICT DO NOTHING
                                                          ▼
                                                     (204 No Content)

[인증이 필요한 모든 요청 — AuthUser extractor]
(요청 + Authorization: Bearer <jwt>)
  │
  ▼
[bearer() 헤더 파싱] ── "Bearer " 접두? ──┬─ 아니오 ─▶ (401)
                                          └─ 예 ─▶ [tokens.verify(token)]  (서명+만료, HS256)
                                                     │
                                                     ├─ 실패 ─▶ (401)
                                                     └─ 성공 → claims(jti,exp) ─▶ [token_revocation.is_revoked(jti)]
                                                                                    │  SELECT EXISTS(...)
                                                                                    ├─ true(철회됨) ─▶ (401)
                                                                                    └─ false ─▶ (AuthUser 통과)
```

> 각 박스가 "왜 그렇게 동작하는가" 는 TECHNICAL 메커니즘 산문 참조.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (stateless 철회 메커니즘·불변조건·실패모드) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (단일 JWT+블랙리스트 선택·대안·근거) | changelog (J-1 ~ J-9, M, G) |
| 무슨 요소를 어떻게 썼나 (jsonwebtoken·sqlx·ureq·async-trait·패턴) | learned |
