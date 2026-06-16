# 학습 기록 (Learned)

> 작성일: 2026-06-13
> 관련 산출물: `docs/plans/2026-06-13/phase11-web-login/task.md`
> 작업 요약: Phase 7 코드 브라우저에 프론트 인증 UI(zustand 토큰 스토어 + axios 인터셉터 + 로그인/회원가입 페이지 + 토프바 로그아웃)를 배선.

> 코드는 모두 Phase 11 종료 스냅샷(`/tmp/cts-snapshots/phase11/tree/...`)에서 직접 복사. 버전은 `frontend/package.json` 기준.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| zustand | ^5.0.9 | 전역 인증 상태(`useAuth`) | 경량·보일러플레이트 적음, React 트리 밖에서도 접근 가능 |
| axios | ^1.13.2 | REST 호출 + 요청 인터셉터로 Bearer 헤더 주입 | 인스턴스/인터셉터로 cross-cutting 헤더를 한 곳에서 처리 |
| react-router-dom | ^7.11.0 | `/login` 라우트, `useNavigate` 프로그래매틱 이동, `Link` | 기존 SPA 라우터(Phase 7)를 그대로 활용 |
| lucide-react | ^0.562.0 | `LogOut` 아이콘(로그아웃 버튼) | 기존 아이콘 세트(GitBranch) 일관 |
| react | ^19.2.0 | `useState`, `React.FormEvent` 타입 | 폼 로컬 상태 |

---

## 2. 핵심 함수 / 메서드

### zustand

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `create` | `create<T>(initializer: (set) => T) => UseBoundStore` | store 생성, `useAuth` 훅 반환 | `stores/index.ts:18` |
| `set` | `(partial: Partial<T>) => void` | store 상태 갱신(리렌더 트리거) | `stores/index.ts:21,26` |
| `useAuth(selector)` | `<U>(s: AuthState) => U` | 선택 구독 | `pages/Login.tsx:11` (`(s) => s.setAuth`) |
| `useAuth()` (전체) | `() => AuthState` | 전체 구독 | `App.tsx:14` (`{ username, clear }`) |

**사용 예시:**
```
import { create } from 'zustand'

const TOKEN_KEY = 'cts_token'
const USER_KEY = 'cts_user'

interface AuthState {
  token: string | null
  username: string | null
  setAuth: (token: string, username: string) => void
  clear: () => void
}

export const useAuth = create<AuthState>((set) => ({
  token: localStorage.getItem(TOKEN_KEY),
  username: localStorage.getItem(USER_KEY),
  setAuth: (token, username) => {
    localStorage.setItem(TOKEN_KEY, token)
    localStorage.setItem(USER_KEY, username)
    set({ token, username })
  },
  clear: () => {
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(USER_KEY)
    set({ token: null, username: null })
  },
}))
```
- 출처: `frontend/src/stores/index.ts:6-31`

**코드 설명:**
> `create<AuthState>((set) => ({...}))` — 초기 상태와 액션을 한 객체로 정의하는 store를 만든다. 초기값을 `localStorage.getItem`으로 읽어 새로고침 시 로그인을 복원한다.
> `set({ token, username })` — 부분 상태를 병합 갱신하고 구독 컴포넌트를 리렌더한다. localStorage 쓰기와 함께 호출해 영속·메모리를 동기화한다.
> `useAuth((s) => s.setAuth)` — selector로 필요한 조각만 구독해 불필요한 리렌더를 피한다(Login은 setAuth만 필요).

### axios

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `axios.create` | `(config) => AxiosInstance` | baseURL 고정 인스턴스 | `services/index.ts:19` |
| `api.interceptors.request.use` | `(onFulfilled: (config) => config) => number` | 발신 요청 가로채 헤더 주입 | `services/index.ts:22-28` |
| `api.post<T>` | `(url, data?) => Promise<AxiosResponse<T>>` | 인증 엔드포인트 호출 | `services/index.ts:39,44,46` |
| `api.get<T>` | `(url, config?) => Promise<AxiosResponse<T>>` | 저장소/트리 조회(헤더 자동 첨부) | `services/index.ts:48~` |

**사용 예시:**
```
const api = axios.create({ baseURL: `${API_BASE}/api` })

// 저장된 토큰을 모든 요청에 Bearer 로 첨부
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('cts_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

export const login = (username: string, password: string) =>
  api.post<AuthResult>('/auth/login', { username, password }).then((r) => r.data)

export const logout = () => api.post('/auth/logout').then((r) => r.data)
```
- 출처: `frontend/src/services/index.ts:19-46`

**코드 설명:**
> `axios.create({ baseURL })` — 모든 호출 앞에 `${API_BASE}/api`를 붙이는 인스턴스. 인터셉터도 이 인스턴스에만 적용된다.
> `interceptors.request.use(fn)` — 요청 전송 직전 `config`를 받아 변형 후 반환. 여기서 `config.headers.Authorization`을 세팅하고, 반드시 `config`를 return해야 요청이 진행된다.
> `api.post<AuthResult>(...).then((r) => r.data)` — 제네릭으로 응답 타입을 지정하고 `.data`만 꺼내 반환. logout은 본문 없이 POST(인터셉터가 토큰을 실어 서버가 jti 식별).

### react-router-dom

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `useNavigate` | `() => (to: string) => void` | 프로그래매틱 이동 | `App.tsx:14`, `Login.tsx:11` |
| `<Route path element>` | JSX | `/login` 라우트 등록 | `App.tsx:58` |
| `<Link to>` | JSX | 선언적 링크 | `App.tsx:30,43` |

**사용 예시:**
```
<Routes>
  <Route path="/" element={<RepoList />} />
  <Route path="/login" element={<Login />} />
  <Route path="/repos/:id" element={<RepoView />} />
</Routes>
```
- 출처: `frontend/src/App.tsx:56-60`

**코드 설명:**
> `useNavigate()` — 호출 시 `navigate('/')`/`navigate('/login')`로 코드에서 라우트 전환(로그인 성공·로그아웃 후).
> `<Route path="/login" element={<Login />} />` — 신규 로그인 페이지 라우트. `TopBar`의 비로그인 링크와 짝.

---

## 3. 어노테이션 / 데코레이터

해당 없음 (TypeScript/React — 데코레이터 미사용).

---

## 4. 수정 전/후 코드 비교

### 파일명: `frontend/src/App.tsx`

**수정 전:**
```
import { BrowserRouter, Link, Route, Routes } from 'react-router-dom'
import { GitBranch } from 'lucide-react'
import RepoList from './pages/RepoList'
import RepoView from './pages/RepoView'

function App() {
  return (
    <BrowserRouter>
      <div className="app">
        <div className="topbar">
          <GitBranch size={20} color="#58a6ff" />
          <Link to="/" className="brand">
            Code<span>Storage</span>
          </Link>
          <span className="muted small">— 독립 버전 관리 시스템</span>
        </div>
        <Routes>
          <Route path="/" element={<RepoList />} />
          <Route path="/repos/:id" element={<RepoView />} />
        </Routes>
      </div>
```

**수정 후:**
```
import { BrowserRouter, Link, Route, Routes, useNavigate } from 'react-router-dom'
import { GitBranch, LogOut } from 'lucide-react'
import RepoList from './pages/RepoList'
import RepoView from './pages/RepoView'
import Login from './pages/Login'
import { useAuth } from './stores'
import { logout as apiLogout } from './services'

function TopBar() {
  const navigate = useNavigate()
  const { username, clear } = useAuth()

  const onLogout = async () => {
    try {
      await apiLogout()
    } catch {
      /* 무시 */
    }
    clear()
    navigate('/login')
  }

  return (
    <div className="topbar">
      <GitBranch size={20} color="#58a6ff" />
      <Link to="/" className="brand">
        Code<span>Storage</span>
      </Link>
      <span className="muted small">— 독립 버전 관리 시스템</span>
      <span className="spacer" />
      {username ? (
        <>
          <span className="small">@{username}</span>
          <button onClick={onLogout} title="로그아웃">
            <LogOut size={13} />
          </button>
        </>
      ) : (
        <Link to="/login" className="small">
          로그인
        </Link>
      )}
    </div>
  )
}

function App() {
  return (
    <BrowserRouter>
      <div className="app">
        <TopBar />
        <Routes>
          <Route path="/" element={<RepoList />} />
          <Route path="/login" element={<Login />} />
          <Route path="/repos/:id" element={<RepoView />} />
        </Routes>
      </div>
```

**변경 이유:** 정적 토프바를 인증 상태(`username`)에 반응하는 `TopBar` 컴포넌트로 승격하고, `/login` 라우트와 로그아웃 흐름을 추가하기 위해. App 본문은 `useNavigate`가 BrowserRouter 자식에서만 동작하므로 TopBar로 훅 사용을 옮겼다.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `App` | 인라인 `.topbar` div → `<TopBar />` 호출 | 토프바에 상태·훅 도입 |
| `TopBar` (신규) | useAuth 구독 + onLogout + 조건 렌더 | 인증 상태 기반 UI 전환 |
| `onLogout` (신규) | apiLogout → clear → navigate | 서버 철회 + 클라 정리 2단계 |

### 파일명: `frontend/src/services/index.ts`

**수정 전:** (인터셉터·인증 API 없음 — `axios.create` 직후 바로 `getRepositories` 등 조회 함수)
```
const api = axios.create({ baseURL: `${API_BASE}/api` })

export const getRepositories = () =>
  api.get<Repository[]>('/repositories').then((r) => r.data)
```

**수정 후:**
```
const api = axios.create({ baseURL: `${API_BASE}/api` })

// 저장된 토큰을 모든 요청에 Bearer 로 첨부
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('cts_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

// -----------------------------------------------------------------------------
// 인증
// -----------------------------------------------------------------------------
export interface AuthResult {
  token: string
  user: { id: string; username: string; email: string }
}

export const register = (username: string, email: string, password: string) =>
  api
    .post<AuthResult>('/auth/register', { username, email, password })
    .then((r) => r.data)

export const login = (username: string, password: string) =>
  api.post<AuthResult>('/auth/login', { username, password }).then((r) => r.data)

export const logout = () => api.post('/auth/logout').then((r) => r.data)

export const getRepositories = () =>
  api.get<Repository[]>('/repositories').then((r) => r.data)
```

**변경 이유:** 기존 조회 함수들이 변경 없이 인증 헤더를 받도록 인터셉터를 인스턴스에 추가하고, 인증 엔드포인트(register/login/logout)를 배선.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `api` 인스턴스 | 요청 인터셉터 1개 추가 | 모든 요청에 Bearer 자동 첨부 |
| `register/login/logout` (신규) | Phase 8·10 엔드포인트 래퍼 | 폼/토프바에서 호출 |

### 파일명: `frontend/src/stores/index.ts`

**수정 전:** (사실상 빈 파일 — 공백 1줄)
```
 
```

**수정 후:** §2 zustand 사용 예시와 동일(`useAuth` 전체 정의).

**변경 이유:** 인증 전역 상태 도입 — 빈 파일을 `useAuth` 스토어로 채움.

### 파일명: `frontend/src/index.css`

**수정 전:** (`input` 규칙 없음 — `button` 규칙 뒤 바로 `button:hover`)
```
  font-size: 13px;
  cursor: pointer;
}
button:hover {
```

**수정 후:**
```
  font-size: 13px;
  cursor: pointer;
}

input {
  background: #0d1117;
  color: #e6e6e6;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px 10px;
  font-size: 14px;
}
input:focus {
  outline: none;
  border-color: var(--accent);
}
button:hover {
```

**변경 이유:** 로그인 폼의 `<input>`이 다크 테마와 일관되도록 스타일 추가(순수 프레젠테이션).

---

## 5. 동작 구조

### 실행 흐름

```
[로그인]
Login 폼 submit
  → services.login/register (mode 분기)
    → axios 인터셉터: localStorage 토큰 있으면 Bearer 첨부 (로그인 시엔 보통 없음)
      → POST /api/auth/{login|register}
    ← AuthResult { token, user }
  → useAuth.setAuth(token, user.username)
    → localStorage.setItem(cts_token/cts_user) + zustand set
  → navigate('/')
  → TopBar 리렌더: @username + 로그아웃 버튼

[인증된 조회]
RepoList → services.getRepositories
  → 인터셉터: localStorage 토큰 → Authorization: Bearer
    → GET /api/repositories (서버가 토큰으로 비공개 포함 필터)

[로그아웃]
TopBar onLogout
  → services.logout → POST /api/auth/logout (인터셉터가 토큰 첨부 → 서버 jti 블랙리스트)
    (실패는 catch 로 무시)
  → useAuth.clear() → localStorage.removeItem + zustand set null
  → navigate('/login') → 공개 저장소만 노출
```

### 컴포넌트별 역할

| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| `TopBar` | `App.tsx` | 인증 상태 UI + 로그아웃 | `useAuth()`, `apiLogout`, `clear`, `navigate` |
| `Login` | `pages/Login.tsx` | 로그인/회원가입 폼 | `login`/`register`, `setAuth`, `navigate` |
| `useAuth` | `stores/index.ts` | 토큰/사용자명 상태+영속 | `localStorage.*`, `set` |
| `api` 인터셉터 | `services/index.ts` | Bearer 헤더 주입 | `localStorage.getItem` |

### 데이터 흐름

```
로그인 입력 (username, password[, email])
  → services.login/register: { ... } 본문 POST
  → 서버 → AuthResult { token, user{ id, username, email } }
  → setAuth(token, user.username)
  → localStorage: cts_token=<JWT>, cts_user=<username>
  → 이후 모든 요청 헤더: Authorization: Bearer <JWT>
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| Interceptor(미들웨어) | `services/index.ts` 요청 인터셉터 | cross-cutting 인증 헤더를 호출부와 분리 | 요청 파이프라인에 함수 1개 삽입 |
| Store/Observer | `stores/index.ts` `useAuth` | 분산 컴포넌트가 같은 상태 구독·리렌더 | zustand store + selector 구독 |
| Facade(서비스 모듈) | `services/index.ts` export 함수들 | HTTP 세부를 가린 도메인 API | `login/logout/getRepositories` 등 |

**패턴 상세:**

### Interceptor
- **의도**: 모든 요청에 공통 처리(인증)를 호출부 수정 없이 적용.
- **구조**: `axios` 인스턴스의 요청 파이프라인에 `(config) => config` 함수를 등록.
- **이 프로젝트에서의 적용**:
```
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('cts_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})
```
- 출처: `frontend/src/services/index.ts:22-28`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| localStorage 토큰 키 | `cts_token` | 인터셉터·스토어 공유(stores는 `TOKEN_KEY` 상수, 인터셉터는 리터럴) |
| localStorage 사용자 키 | `cts_user` | 토프바 사용자명 표시·복원 |
| API base | `VITE_API_URL ?? http://127.0.0.1:8080` + `/api` | env 미설정 시 로컬 폴백 |
| 토큰 저장 위치 | 브라우저 `localStorage` (평문 JWT) | SPA 영속 — XSS 노출 트레이드오프 수용(`TECHNICAL §외부 경계`) |
| 인증 헤더 형식 | `Authorization: Bearer <JWT>` | Phase 8 서버 규약 |

---

## 8. 테스트에서 사용된 것들

해당 없음 — 이 작업은 자동화 테스트를 추가하지 않았다. 검증은 `tsc -b && vite build` 타입 통과(에러 0)와 수동 시나리오(회원가입/로그인→토큰 저장·토프바 표시·비공개 노출, 로그아웃→공개만)로 갈음했다. 인증 API 자체의 동작은 Phase 8·10에서 이미 검증됨(task.md §결과).

---

## 9. 새로 알게 된 것

- **axios 인터셉터는 React 훅을 못 쓴다.** 인터셉터는 렌더 사이클 밖 순수 함수라 `useAuth()`를 호출할 수 없어, 토큰을 zustand 스토어가 아닌 `localStorage`에서 직접 읽는다. 그래서 진실 소스가 사실상 `localStorage`가 되고 스토어는 렌더 표시용 파생값에 가깝다.
- **로그아웃의 빈 `catch {}`는 버그가 아니라 best-effort 설계.** 서버 jti 철회(Phase 10) 호출이 실패해도 클라이언트 토큰은 무조건 비워야 사용자가 로그아웃된 것으로 보인다. 단, 그 결과로 서버 측에서 토큰이 만료까지 살아있을 수 있다.
- **localStorage 평문 JWT의 XSS 노출.** HttpOnly 쿠키와 달리 `localStorage`는 같은 오리진 JS가 모두 읽으므로 XSS 시 토큰이 탈취된다. 이 작업은 SPA 단순성을 위해 그 위험을 수용했다.
- **응답 인터셉터가 없다.** 만료/철회된 토큰으로 호출하면 서버가 401을 주지만 자동 로그아웃/리다이렉트가 없어 해당 요청만 실패한다.

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| HttpOnly 쿠키 vs localStorage 토큰 저장 | XSS/CSRF 트레이드오프 — 현재는 localStorage(XSS 노출) | OWASP Token Storage |
| axios 응답 인터셉터 + 401 자동 로그아웃 | 만료/철회 토큰 처리 자동화(현재 누락) | axios interceptors 문서 |
| zustand `persist` 미들웨어 | 수동 localStorage 동기화를 대체할 수 있는가 | zustand persist 문서 |
| 다중 탭 storage 이벤트 동기화 | 한 탭 로그아웃이 다른 탭에 반영되지 않는 한계 | `window.addEventListener('storage')` |
