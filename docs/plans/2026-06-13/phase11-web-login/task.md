# Phase 11 — Web UI 로그인

## 범위
프론트엔드에 로그인/회원가입 추가 → 토큰 저장 → API 호출에 자동 첨부.
로그인 시 비공개 저장소도 목록/조회 가능(서버가 토큰으로 필터).

## 설계
- 상태: zustand 스토어(`useAuth`) — token/username, localStorage 영속.
- axios 인터셉터: localStorage 토큰을 `Authorization: Bearer` 로 첨부.
- 페이지: `/login` — 로그인/회원가입 토글 폼.
- 토프바: 로그인 시 사용자명 + 로그아웃 버튼, 아니면 "로그인" 링크.
- 로그아웃: POST /auth/logout + 스토어/로컬 정리.

## 구현 (커밋 단위)
1. 프론트 인증: stores/auth(zustand) + services(인터셉터+auth API) +
   pages/Login + App(라우트/토프바)
2. docs/로드맵

## 검증(예정)
- 회원가입/로그인 → 토큰 저장, 토프바 사용자 표시, 비공개 저장소 목록 노출,
  로그아웃 → 공개만, tsc/vite build 통과.

## 결과 (2026-06-13)
- ✅ tsc -b && vite build 통과(타입 에러 0).
- ✅ 인증 API(register/login/logout)는 Phase 8·10 에서 검증됨 — 프론트는 이를 배선.
- 인터셉터가 localStorage 토큰을 Bearer 로 첨부 → 로그인 시 비공개 저장소 노출,
  로그아웃 시 토큰 철회 + 공개만.
