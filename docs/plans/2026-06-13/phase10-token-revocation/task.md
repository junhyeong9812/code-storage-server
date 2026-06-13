# Phase 10 — 토큰 철회 / 로그아웃 (단일 JWT + 블랙리스트)

## 결정
- access/refresh 회전 없이 **단일 JWT 유지** + **jti 블랙리스트(DB)** 로 철회.
- 로그아웃 = 해당 토큰의 jti 를 철회 목록에 추가.
- 검증 시: 서명·만료 확인 후 jti 가 철회됐는지 추가 확인.
- 만료(30일) 시 재로그인. (refresh 재발급은 이 모델에 없음 — 사용자 선택)

## 스키마 (init.sql + 실행 DB 적용)
```sql
CREATE TABLE revoked_tokens (
  jti VARCHAR(64) PRIMARY KEY,
  expires_at TIMESTAMPTZ NOT NULL,   -- 만료 후 정리용
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## 설계
- JWT Claims 에 `jti`(랜덤 UUID) 추가. AuthClaims/AuthUser 가 jti·exp 보유.
- 포트 `TokenRevocation`: is_revoked(jti) / revoke(jti, expires_at).
- AuthUser 추출기: TokenService.verify → claims → revocation.is_revoked → 철회면 401.
- 로그아웃: POST /api/auth/logout (인증) → revoke(jti, exp).
- CLI: `cts logout <server_url>` → 서버 철회 + 전역 자격증명에서 제거.

## 구현 (커밋 단위)
1. 철회 인프라: schema + TokenRevocation 포트/어댑터 + JWT jti +
   AuthClaims/AuthUser(jti,exp) + 추출기 철회 검사 + AppState/main
2. 로그아웃 API (use_case + handler + route)
3. CLI cts logout + docs/로드맵

## 결과 (2026-06-13)
- ✅ `cargo test` 전체 green (57). 스키마는 init.sql + 실행 DB 적용.
- ✅ 서버 E2E: /me 200 → logout 204 → 같은 토큰 /me 401, revoked_tokens 1행, 재로그인 200.
- ✅ CLI E2E: cts logout → 전역 자격증명 비고 + 이전 토큰 서버 401.

## 한계 / 후속
- 단일 기기/토큰 단위 철회만(전체 세션 무효화 X). 만료 행 정리(cron) 미구현.
- access/refresh 회전 없음(사용자 선택) — 토큰 수명은 30일 고정.
</content>
