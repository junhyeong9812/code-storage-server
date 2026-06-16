# changelog: Phase 11 — Web UI 로그인/로그아웃

> 이번 diff의 의사결정 로그. 코드 블록은 Phase 11 종료 스냅샷(`/tmp/cts-snapshots/phase11/tree/...`)에서 그대로 복사 — 블록 안 해설 주석 없음, 해설은 라인별 근거 표로.

**검증 상태**: 통과 (실행 명령: `tsc -b && vite build`, 타입 에러 0 — 출처 task.md §결과 2026-06-13. 자동화 테스트 없음, 인증 API 자체는 Phase 8·10에서 검증됨).

**대상 diff 파일** (`_namestatus.txt`, 프로세스 문서 task.md 제외):
README.md(M), frontend/src/App.tsx(M), frontend/src/index.css(M), frontend/src/pages/Login.tsx(A), frontend/src/services/index.ts(M), frontend/src/stores/index.ts(M).

## 1. 판단 항목 (J)

### J-1: 토프바를 인증 상태 의존 컴포넌트로 분리하고 로그아웃을 2단계로 처리 — `frontend/src/App.tsx`

- **왜**: 토프바가 로그인 여부(`username`)에 따라 다른 UI를 보여야 하므로, 정적 `<div className="topbar">`를 `useNavigate`/`useAuth` 훅을 쓸 수 있는 별도 컴포넌트 `TopBar`로 분리했다. 로그아웃은 서버 토큰 철회(Phase 10) 호출 후 클라이언트 정리 순서로 묶었다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | TopBar 컴포넌트 분리 (선택) | 훅 사용 가능, 상태 구독으로 자동 리렌더 | 컴포넌트 1개 추가 | 선택 — 토프바가 스토어를 구독해야 로그인 후 즉시 갱신 |
  | App 본문에서 인라인 조건 렌더 | 파일 단순 | App이 useNavigate를 직접 쓰려면 BrowserRouter 안이어야 함 + 책임 혼재 | 기각 |
  | 로그아웃 시 서버 호출 생략, 클라 토큰만 삭제 | 더 단순 | jti 블랙리스트(Phase 10) 미활용 → 토큰 만료까지 서버서 유효 | 기각 — 철회 API 활용이 Phase 목적 |
- **근거 출처**: task.md §설계("토프바: 로그인 시 사용자명 + 로그아웃 버튼", "로그아웃: POST /auth/logout + 스토어/로컬 정리").
- **코드**:
  ```
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
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `const { username, clear } = useAuth()` | 스토어 구독 — `username` 변경 시 TopBar 리렌더가 로그인/로그아웃 UI 전환을 일으킴 |
  | `await apiLogout()` … `catch { /* 무시 */ }` | 서버 jti 철회는 best-effort — 실패해도 클라이언트 로그아웃을 막지 않음(불변조건: 로그아웃은 항상 완료). 메커니즘 `TECHNICAL §실패 모드` |
  | `clear(); navigate('/login')` | 서버 호출 결과와 무관하게 스토어/localStorage 정리 후 이동 |
  | `{username ? (…) : (…)}` | 인증 상태 분기 — 토큰 유무가 아니라 `username`으로 판정 |
- **리뷰 연습 포인트**:
  - 컴포넌트 경계 렌즈 — `onLogout`의 `catch`가 비어 있다. 의도적 무시인가 누락인가? 서버 철회 실패를 사용자에게 알릴 필요는 없는가?
  - 인증 분기 렌즈 — `username`만으로 로그인 판정하는데, `token`은 있고 `username`만 없는 불일치 상태가 가능한가?(`setAuth`/`clear`가 둘을 항상 함께 다루는지)

### J-2: axios 요청 인터셉터로 Bearer 헤더를 전역 주입 + 인증 API 추가 — `frontend/src/services/index.ts`

- **왜**: 모든 API 함수에 토큰 첨부를 반복하지 않기 위해 단일 `api` 인스턴스에 요청 인터셉터를 등록했다. 토큰 소스는 `localStorage`를 직접 읽는다(인터셉터는 React 훅 호출 불가).
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 요청 인터셉터 + localStorage 직접 읽기 (선택) | 한 곳에서 모든 요청 커버, 훅 불필요 | 스토어와 진실 소스 이원화 | 선택 — cross-cutting 헤더에 표준적 |
  | 함수마다 헤더 인자 전달 | 명시적 | 모든 호출부 수정·누락 위험 | 기각 |
  | 인터셉터에서 zustand store.getState() 읽기 | 단일 소스 | 인터셉터-스토어 결합도 상승, 초기 로드 타이밍 의존 | 기각 — localStorage가 더 단순 |
- **근거 출처**: task.md §설계("axios 인터셉터: localStorage 토큰을 `Authorization: Bearer` 로 첨부").
- **코드**:
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
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `localStorage.getItem('cts_token')` | 진실 소스 = localStorage. 키 문자열이 stores의 `TOKEN_KEY`와 일치해야 함(불변조건) |
  | `if (token) { ... Authorization ... }` | 토큰 없으면 헤더 미첨부 → 익명 요청 → 서버가 공개 저장소만 반환 |
  | `AuthResult { token, user{...} }` | login/register 공통 응답 셰이프 — Login.submit이 둘을 동일 처리 |
  | `register/login/logout` | Phase 8·10 서버 엔드포인트 배선. logout은 본문 없이 POST(인터셉터가 토큰 첨부 → 서버가 jti 식별) |
- **리뷰 연습 포인트**:
  - 외부 경계 렌즈 — 401 응답을 가로채는 응답 인터셉터가 없다. 만료/철회 토큰으로 호출 시 자동 로그아웃이 필요한가?
  - 보안 렌즈 — 토큰이 localStorage에 평문 저장된다. XSS 노출 트레이드오프는 명시적으로 수용됐나?(`TECHNICAL §외부 경계`)

### J-3: 토큰/사용자명을 localStorage 영속 zustand 스토어로 관리 — `frontend/src/stores/index.ts`

- **왜**: 떨어진 컴포넌트(TopBar, Login)가 인증 상태를 공유하고, 새로고침 후에도 로그인을 유지해야 했다. 빈 파일이던 `stores/index.ts`에 `useAuth`를 신설했다.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | zustand + localStorage 수동 동기화 (선택) | 경량, 보일러플레이트 적음, 영속 명시적 | 영속 로직 직접 작성 | 선택 — task.md 지정 |
  | zustand persist 미들웨어 | 영속 자동화 | 직렬화 포맷 추상화, 인터셉터가 읽을 키 제어 약화 | 기각 — 인터셉터가 raw 키를 읽어야 함 |
  | React Context + useReducer | 의존성 0 | 보일러플레이트, 외부(인터셉터)서 접근 불가 | 기각 |
  | sessionStorage | 탭 종료 시 자동 정리 | 새 탭/재방문 시 로그인 풀림 | 기각 — 영속 요구 |
- **근거 출처**: task.md §설계("상태: zustand 스토어(`useAuth`) — token/username, localStorage 영속").
- **코드**:
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
  | 줄 | 근거 해설 |
  |----|----------|
  | `token: localStorage.getItem(TOKEN_KEY)` | 스토어 초기값을 localStorage에서 복원 → 새로고침 후 로그인 유지 |
  | `TOKEN_KEY = 'cts_token'` | 인터셉터가 쓰는 리터럴과 동일해야 함(현재 인터셉터는 리터럴 하드코딩 — 동기화 책임 분산) |
  | `setAuth`: setItem + set | localStorage(진실 소스)와 스토어(렌더용)를 원자적으로 함께 갱신(불변조건) |
  | `clear`: removeItem + set null | 로그아웃 시 양쪽 동시 정리 — 유령 인증 방지 |
- **리뷰 연습 포인트**:
  - 상태 소유권 렌즈 — `TOKEN_KEY` 상수가 stores에 있는데 인터셉터는 `'cts_token'` 리터럴을 쓴다. 키 동기화가 컴파일 타임에 강제되나?
  - 동시성/타이밍 렌즈 — 다른 탭에서 로그아웃 시 이 탭의 스토어는 갱신되지 않는다(storage 이벤트 미구독). 의도된 한계인가?

### J-4: 로그인↔회원가입 토글 폼 페이지 신설 — `frontend/src/pages/Login.tsx`

- **왜**: `/login` 한 화면에서 로그인과 회원가입을 `mode` 상태로 토글해, 폼 필드(이메일)와 호출 API를 분기한다. 성공 시 `setAuth` 후 홈으로 이동.
- **대안 비교**:
  | 접근 | 장점 | 단점 | 선택/기각 사유 |
  |------|------|------|---------------|
  | 단일 페이지 mode 토글 (선택) | 라우트 1개, 코드 공유 | 한 컴포넌트가 두 책임 | 선택 — task.md "로그인/회원가입 토글 폼" |
  | /login·/register 별도 라우트 | 책임 분리 | 중복 폼·중복 submit 로직 | 기각 |
- **근거 출처**: task.md §설계("페이지: `/login` — 로그인/회원가입 토글 폼").
- **코드**:
  ```
  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const result =
        mode === 'login'
          ? await login(username, password)
          : await register(username, email, password)
      setAuth(result.token, result.user.username)
      navigate('/')
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { error?: string } } })?.response?.data
          ?.error ?? '실패했습니다'
      setError(msg)
    } finally {
      setBusy(false)
    }
  }
  ```
  | 줄 | 근거 해설 |
  |----|----------|
  | `mode === 'login' ? login(...) : register(...)` | 단일 핸들러가 모드별 API 분기 — 두 응답이 동일 `AuthResult`라 가능(불변조건) |
  | `setAuth(result.token, result.user.username)` | 토큰+사용자명을 스토어/localStorage에 영속 → 토프바 즉시 갱신 |
  | `(err as {...})?.response?.data?.error ?? '실패했습니다'` | 서버 에러 메시지를 옵셔널 체이닝으로 안전 추출, 없으면 폴백 |
  | `finally { setBusy(false) }` | 성공/실패 무관 버튼 재활성화 — 중복 제출 가드(`disabled={busy}`) 해제 |
- **리뷰 연습 포인트**:
  - 입력 검증 렌즈 — 클라이언트 측 필드 검증(빈 값·이메일 형식)이 없다. 서버 에러에만 의존하는 게 적절한가?
  - 에러 처리 렌즈 — `err`를 인라인 타입 단언으로 좁힌다. 네트워크 단절처럼 `response`가 없는 에러도 `'실패했습니다'`로 잘 폴백되나?

## 2. 기계적 변경 (M — 1줄 + 동작 동일 근거)

- `frontend/src/index.css` — `input`/`input:focus` 스타일 규칙 추가. 동작 동일 근거: 순수 CSS 프레젠테이션(배경·테두리·포커스 색), JS 동작/로직 영향 없음. 로그인 폼 input의 시각적 일관성용.
- `README.md` — 로드맵에 `- [x] Phase 11: Web UI 로그인 …` 한 줄 추가. 동작 동일 근거: 문서 텍스트, 코드/빌드 무영향.
- `docs/plans/2026-06-13/phase{2,3,4,5,6,7,8,9,10}/task.md` — 각 파일 끝의 잘못 들어간 `</content>` 아티팩트 1줄 삭제(커밋 92b71dc). 동작 동일 근거: 프로세스 문서 텍스트 정리, 산출물/빌드 무영향. (커버리지 규칙상 task.md=프로세스 문서로 본 작업의 J/M/G 대상에서 제외되나, 투명성을 위해 기재.)

## 3. 생성물 (G)

- 없음 — lockfile/generated/snapshot 변경 없음(`package.json` 의존성 추가 없이 기존 zustand/axios/react-router-dom 사용).

---

**셀프체크 □**: 대상 diff의 비-프로세스 파일 6개 전수 분류 완료 — App.tsx(J-1), services/index.ts(J-2), stores/index.ts(J-3), pages/Login.tsx(J-4), index.css(M), README.md(M). 프로세스 문서(phase*/task.md)는 커버리지 제외이나 M에 부기. 누락 0.
