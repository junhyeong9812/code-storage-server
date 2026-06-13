# Phase 8 — 인증/인가 (User)

> 처음 검증 때 유보했던 User/Auth 단계. 시드 유저 고정 → 실제 인증으로 교체.

## 결정
- **토큰**: JWT (jsonwebtoken, JWT_SECRET env, 만료 포함). 상태없음.
- **비밀번호 해싱**: bcrypt (통합 단순·검증됨).
- **인가**: 공개읽기 + 소유자쓰기. is_private 존중(비공개는 소유자만 읽기).
- **CLI**: register/login + 전역 자격증명(~/.config/cts/credentials.json, 서버 URL별).

## 서버
- user 도메인: UserId/Email/Username/User 엔티티, UserRepository 포트,
  PasswordHasher/TokenService 포트
- 인프라: PgUserRepository, BcryptPasswordHasher, JwtTokenService
- 애플리케이션: register/login 유스케이스, DTO(Register/Login/AuthResponse)
- API: POST /api/auth/register, POST /api/auth/login
- AuthUser 추출기(Authorization: Bearer) + MaybeAuthUser(옵션)
- AppError::Forbidden(403) 추가
- 인가 적용: create/delete/push/build = 소유자, read/pull = 공개 or 소유자

## CLI
- credentials.rs: 전역 자격증명 저장/로드(서버 URL별 토큰)
- cts register <url> <username> <email> / cts login <url> <username>
  (비밀번호는 rpassword 로 숨김 입력)
- remote/push/pull/clone: 해당 서버 토큰을 Authorization 헤더로 전송

## 커밋 단위
1. deps + AppError::Forbidden
2. server user 도메인+애플리케이션+인프라
3. server auth API + AuthUser 추출기 + 배선
4. server 인가 적용(repository/build 핸들러)
5. CLI 인증(credentials, login/register, 토큰 전송)
6. docs/로드맵

## 결과 (2026-06-13)
- ✅ `cargo test` 전체 green: cli 2 + core 25 + server 12 + doctest 18 = 57
  (server: email/username 값객체 단위 테스트 추가, Email local 공백 버그 수정)
- ✅ 서버 인증 E2E: register 201 / 중복 409 / login / 오답 401 / me 200·401
- ✅ 서버 인가 E2E: 무토큰 create 401, 비소유자 delete 403,
  비공개 repo 익명·타인 404 은닉, 목록 가시성(소유자만 본인 비공개)
- ✅ CLI E2E: alice login→remote→push 성공(owner=alice), bob push→403,
  bob pull(공개)→성공, 재push 멱등
- 비밀번호 입력: rpassword 숨김 + CTS_PASSWORD env 비대화 폴백(CI/테스트)

## 한계 / 후속
- 토큰 갱신/로그아웃·만료 회전 없음(JWT 30일 고정). revoke 불가(상태없음).
- Web UI 는 로그인 없이 공개 저장소만 조회(설계상). 비공개/쓰기 UI 없음.
- 시드 testuser(init.sql)는 더 이상 사용 안 함(로그인 불가 dummy 해시) — 무해.
- repo 협업자(read/write 공유) 개념 없음 — 소유자 단독.
</content>
