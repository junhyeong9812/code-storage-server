# 09. 프론트엔드 (Web UI)

[← 08 CLI](08-cli.md) | [인덱스](README.md) | [다음: 10 데이터베이스 →](10-database.md)

React(Vite) 단일 페이지 앱. 서버의 **읽기 API**를 소비하는 코드 브라우저 + 로그인. 코드: `frontend/src/`.

## 구조

| 파일 | 역할 |
|------|------|
| `main.tsx` | 진입점(React 마운트) |
| `App.tsx` | 라우터 + 토프바(로그인 상태/로그아웃) |
| `pages/RepoList.tsx` | 저장소 목록 |
| `pages/RepoView.tsx` | 저장소 뷰: 브랜치 선택 → 커밋 → 파일 브라우저 + 빌드 패널 |
| `pages/Login.tsx` | 로그인/회원가입 토글 폼 |
| `services/index.ts` | axios 클라이언트 + 인터셉터 + API 함수 |
| `stores/index.ts` | zustand 인증 스토어(token/username, localStorage) |
| `types/index.ts` | 서버 DTO 대응 타입 |

## 라우팅
```
/            RepoList   (저장소 목록)
/login       Login      (로그인/회원가입)
/repos/:id   RepoView   (코드 브라우저)
```

## 인증 흐름 (Phase 11)
1. `Login`에서 register/login → `{token, user}` 수신.
2. `useAuth.setAuth(token, username)` → zustand 상태 + `localStorage`(`cts_token`,`cts_user`).
3. axios **요청 인터셉터**가 `localStorage`의 토큰을 모든 요청에 `Authorization: Bearer`로 첨부.
4. 로그인 시 서버가 토큰으로 비공개 저장소도 목록/조회 허용.
5. 토프바 로그아웃 → `POST /auth/logout` + 스토어/로컬 정리.

```ts
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('cts_token')
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})
```

## RepoView — 코드 브라우저
- **브랜치 선택**(드롭다운) → 해당 브랜치 커밋 히스토리 로드.
- **커밋 클릭** → 그 커밋을 기준으로 파일 트리 브라우징.
- **FileBrowser**: 현재 경로 상태로 트리 엔트리 표시. 디렉토리 클릭 → 진입(브레드크럼), 파일 클릭 → 내용 표시.
- **BuildsPanel**: 빌드 목록(상태 배지), HEAD 빌드 트리거 버튼, 빌드 클릭 → 로그.

소비하는 서버 엔드포인트: `GET /repositories`, `/:id`, `/:id/branches`, `/:id/commits`, `/:id/tree/:commit`, `/:id/blob/:hash`, `/:id/builds`(+`/:bid/log`).

## 서버 연동 메모
- 기본 API 주소 `http://127.0.0.1:8080`(env `VITE_API_URL`로 변경).
- 서버는 `CorsLayer::permissive()`로 Vite 개발서버(:5173) 교차출처 허용.
- 빌드/타입: `npm run build`(`tsc -b && vite build`) 통과.

## 한계
- 비공개/쓰기 관리 UI 없음(저장소 생성·push는 CLI). diff 뷰/구문 강조 미적용.

[← 08 CLI](08-cli.md) | [인덱스](README.md) | [다음: 10 데이터베이스 →](10-database.md)
