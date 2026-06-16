# OVERVIEW: Phase 11 — Web UI 로그인/로그아웃

> 목적: Phase 7 코드 브라우저(공개 저장소만 조회) 위에 인증 UI를 얹어, 로그인한 사용자가 비공개 저장소까지 보고 로그아웃할 수 있게 한 프론트엔드 작업의 추상 지도. 서버 인증 내부(Phase 8·10)는 다루지 않는다 — 프론트는 그 API의 **클라이언트**다.

## 주요 포인트 (3~7)

- **토큰을 `localStorage`에 영속하는 zustand 스토어(`useAuth`)** — `cts_token`/`cts_user` 키. 새로고침해도 로그인 유지. 까다로운 점: 토큰이 JS에서 읽히는 `localStorage`에 평문 저장돼 XSS에 노출된다. → 메커니즘 `TECHNICAL §상태와 소유권`, `TECHNICAL §외부 경계`
- **axios 요청 인터셉터가 모든 API 호출에 `Authorization: Bearer` 헤더를 자동 첨부** — 토큰 소스는 스토어가 아니라 `localStorage`를 직접 읽는다. 까다로운 점: 스토어 상태와 인터셉터의 진실 소스가 둘로 갈린다. → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-2`
- **`/login` 페이지가 로그인↔회원가입을 한 폼으로 토글** — `mode` 상태로 필드(이메일)와 호출 API를 분기. 까다로운 점: 두 모드가 같은 `submit` 핸들러를 공유하며 결과 셰이프(`AuthResult`)가 동일해야 한다. → 선택 이유 `changelog J-4`
- **로그아웃은 서버 호출 + 클라이언트 정리의 2단계** — `POST /auth/logout`(jti 블랙리스트, Phase 10)을 먼저 호출하고 실패해도 무시한 뒤 `clear()`로 스토어/`localStorage`를 비운다. 까다로운 점: 서버 호출 실패가 클라이언트 로그아웃을 막으면 안 된다. → 메커니즘 `TECHNICAL §실패 모드`, 선택 이유 `changelog J-1`
- **토프바가 인증 상태에 따라 UI를 전환** — `username` 유무로 "@사용자명 + 로그아웃 버튼" 또는 "로그인 링크"를 렌더. → 선택 이유 `changelog J-1`

## 워크플로우 (절차 + 분기)

```
(앱 로드)
  │
  ▼
[useAuth 초기화: localStorage 에서 token/username 복원]
  │
  ▼
[TopBar 렌더] ── username 있나? ──┬─ 예 ─▶ [@username + 로그아웃 버튼]
                                  └─ 아니오 ─▶ [로그인 링크 → /login]

(/login 진입)
  │
  ▼
[Login 폼] ── mode? ──┬─ login ──▶ login(username, password) ──▶ POST /api/auth/login
                      └─ register ▶ register(username,email,password) ▶ POST /api/auth/register
  │
  ▼
(요청) ── axios 인터셉터: localStorage 토큰 있으면 Authorization: Bearer 첨부
  │
  ▼
[응답] ──┬─ 성공 ─▶ setAuth(token, user.username) ─▶ localStorage 저장 + 스토어 갱신 ─▶ navigate('/')
         │                                                  │
         │                                                  ▼
         │                                    [이후 모든 API 호출에 Bearer 첨부 → 비공개 저장소 노출]
         └─ 실패 ─▶ err.response.data.error 추출 ─▶ setError(msg) ─▶ 폼에 빨간 에러 표시

(로그아웃 클릭)
  │
  ▼
[onLogout] ─▶ POST /api/auth/logout ──┬─ 성공/실패 무관 ─▶ clear() (localStorage 삭제 + 스토어 null)
                                       │                       │
                  (실패는 catch 로 무시)                        ▼
                                                          navigate('/login') ─▶ [공개 저장소만 노출]
```

> 각 박스가 **왜 그렇게 동작하는가**(인터셉터의 진실 소스, 로그아웃 best-effort, localStorage 노출)는 TECHNICAL 메커니즘 산문 참조.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (토큰 저장소·인터셉터·실패 모드) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거) | changelog (J-1~J-4, M) |
| 무슨 요소를 어떻게 썼나 (zustand·axios·react-router) | learned |
