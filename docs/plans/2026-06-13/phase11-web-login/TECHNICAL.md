# TECHNICAL: Phase 11 — Web UI 로그인/로그아웃

> 목적: 이 프론트엔드 인증 배선의 diff 비종속 동작 모델 — 클라이언트가 JWT를 어떻게 보관·전송·폐기하는가, 어떤 불변조건이 깨지면 무슨 증상이 나오는가. 서버 측 JWT 검증·jti 블랙리스트 메커니즘(Phase 8·10)은 범위 밖이며, 여기서는 그 API를 "신뢰 경계 밖의 외부 서비스"로만 취급한다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 클라이언트 토큰 관리 (stateless JWT를 브라우저가 보관)

① JWT 기반 인증에서 서버는 세션을 저장하지 않고, 로그인 응답으로 받은 서명 토큰을 클라이언트가 보관했다가 매 요청에 실어 보낸다. ② 이 작업은 SPA(React)라 페이지 새로고침·라우팅 사이에서도 토큰이 살아남아야 했고, 그래서 메모리가 아닌 `localStorage`에 영속한다. ③ 모르면 새로고침마다 로그인이 풀리거나(영속 누락), 반대로 토큰 폐기 시점을 놓쳐(로그아웃 미정리) 철회된 토큰이 계속 전송되는 결함이 난다.

### 개념 2: axios 요청 인터셉터 (cross-cutting 헤더 주입)

① axios 인스턴스는 `interceptors.request.use(fn)`로 모든 발신 요청 직전에 config를 가로채 변형하는 훅을 건다. ② 모든 API 함수에 토큰 첨부 코드를 반복하지 않고 한 곳에서 `Authorization` 헤더를 주입하기 위해 썼다. ③ 모르면 일부 호출에만 헤더가 붙어, 같은 토큰인데 어떤 요청은 인증되고 어떤 요청은 401이 나는 비일관 버그가 생긴다.

### 개념 3: zustand 전역 스토어 (React 외부 상태)

① zustand의 `create`는 React 컴포넌트 트리 밖에 사는 store를 만들고, 컴포넌트는 `useAuth(selector)` 훅으로 구독해 값이 바뀌면 리렌더된다. ② 토프바·로그인 페이지 등 떨어진 컴포넌트가 같은 인증 상태(`username`)를 공유·반영해야 해서 전역 스토어가 필요했다. ③ 모르면 prop drilling이나 Context 보일러플레이트로 상태를 끌고 다니게 되고, 로그인 후 토프바가 갱신되지 않는 stale UI가 난다.

## 동작 방식

**토큰의 진실 소스가 둘로 나뉜다.** zustand 스토어(`token`/`username`)는 *UI 렌더링용* 상태이고, axios 인터셉터는 토큰을 스토어가 아니라 `localStorage.getItem('cts_token')`로 *매 요청마다 직접* 읽는다. 두 경로 모두 `setAuth`가 동시에 갱신(`localStorage.setItem` + `set`)하고 `clear`가 동시에 비우기(`removeItem` + `set null`) 때문에 정상 흐름에서는 일치한다. 인터셉터가 스토어 대신 `localStorage`를 읽는 이유는, 인터셉터가 React 렌더 사이클 밖의 순수 함수여서 훅(`useAuth`)을 호출할 수 없기 때문이다 — `localStorage`는 동기 전역이라 어디서나 읽힌다.

**인증 헤더 주입 시점.** `api`는 단일 axios 인스턴스(`baseURL: ${API_BASE}/api`)이고, 등록된 요청 인터셉터가 전송 직전 `config.headers.Authorization = \`Bearer ${token}\``를 세팅한다. 토큰이 없으면(`if (token)`) 헤더를 붙이지 않으므로, 비로그인 상태의 호출은 익명 요청이 되어 서버가 공개 저장소만 반환한다. 로그인 후에는 동일 인스턴스를 쓰는 모든 함수(`getRepositories` 등)에 자동으로 헤더가 실린다 — 이것이 "로그인하면 비공개 저장소가 보이는" 메커니즘의 클라이언트 측 절반이다(나머지 절반인 토큰 기반 필터링은 서버 책임).

**로그인 성공의 상태 전이.** `login`/`register`는 `AuthResult { token, user }`를 resolve하고, `Login.submit`이 `setAuth(result.token, result.user.username)`을 호출한다. `setAuth`가 `localStorage`와 스토어를 동시에 채우면, 스토어를 구독하던 `TopBar`가 `username` 변화로 리렌더되어 로그인 링크가 "@username + 로그아웃 버튼"으로 바뀐다.

## 불변조건 / 계약 (해당 시)

- `localStorage`의 `cts_token`/`cts_user`와 zustand 스토어의 `token`/`username`은 항상 함께 세팅·삭제된다(`setAuth`/`clear`가 유일한 변경 경로). 깨지면: 토프바는 로그아웃 상태인데 인터셉터는 여전히 토큰을 첨부(또는 반대)하는 유령 인증.
- `login`과 `register`는 동일한 `AuthResult` 셰이프(`token` + `user.username`)를 반환해야 한다 — `Login.submit`이 두 경로의 결과를 같은 `setAuth` 인자로 쓰기 때문. 깨지면: 한 모드에서 `result.user.username`이 `undefined`가 되어 토프바에 빈 사용자명.
- 인터셉터의 토큰 키 문자열 `'cts_token'`은 스토어의 `TOKEN_KEY` 상수와 일치해야 한다. 깨지면: 저장은 되는데 첨부가 안 되어 로그인했는데도 401.

## 상태와 소유권 (해당 시)

- **토큰/사용자명의 source of truth = `localStorage`** (`cts_token`, `cts_user`). 페이지 로드 시 zustand 스토어가 `localStorage`에서 초기값을 읽어(`token: localStorage.getItem(TOKEN_KEY)`) 파생 상태로 들고, 이후 `setAuth`/`clear`가 양쪽을 동기화한다.
- **갱신 주체**: 오직 `useAuth`의 `setAuth`(로그인/회원가입 성공 시 `Login`이 호출)와 `clear`(로그아웃 시 `TopBar`가 호출).
- 인터셉터가 쓰는 토큰은 스토어 파생값이 아니라 `localStorage` 원본을 직접 읽으므로, 인터셉터 관점의 진실 소스도 `localStorage`다.

## 외부 경계와 의존성 (해당 시)

- **브라우저 `localStorage`** — 동기 영속 key-value. 신뢰 수준: 낮음. 같은 오리진의 **모든 JS가 읽을 수 있어** 토큰이 XSS에 노출된다(HttpOnly 쿠키와 달리 스크립트 차단 불가). 평문 JWT가 그대로 저장되며 이 작업은 그 트레이드오프(SPA 단순성 ↔ XSS 노출)를 수용했다. 실패 모드: 시크릿 모드/스토리지 비활성 시 throw 가능성.
- **서버 인증 API** (`POST /api/auth/register`, `/auth/login`, `/auth/logout`) — Phase 8·10이 제공하는 외부 서비스. 신뢰 수준: 프론트는 응답 셰이프(`AuthResult`, 에러는 `response.data.error`)에만 의존. 실패 모드: 4xx/5xx, 네트워크 단절 → axios가 reject.
- **`import.meta.env.VITE_API_URL`** — 빌드/런타임 env. 미설정 시 `http://127.0.0.1:8080` 폴백.

## 실패 모드 메커니즘 (해당 시)

- **로그인/회원가입 실패** (잘못된 자격증명·중복 사용자·서버 4xx): 서버가 에러 응답을 주면 axios가 reject → `submit`의 `catch`가 `err.response.data.error`를 옵셔널 체이닝으로 추출하고, 없으면 `'실패했습니다'`로 폴백해 `setError(msg)`. 증상: 폼 하단에 `--red` 색 메시지. 시스템 반응: 토큰은 세팅되지 않고(`setAuth` 미호출) 폼에 머문다. `finally`가 `busy`를 풀어 버튼이 다시 활성화된다.
- **로그아웃 시 서버 호출 실패** (`POST /auth/logout`가 네트워크/서버 오류): `onLogout`의 `try/catch`가 에러를 삼키고(`/* 무시 */`) 그대로 `clear()`로 진행한다. 이유 — 서버 측 jti 블랙리스트 등록이 실패해도 클라이언트는 토큰을 버려야 사용자가 로그아웃된 것으로 보이기 때문(best-effort 폐기). 증상: 클라이언트는 로그아웃되지만 서버에선 토큰이 만료까지 유효할 수 있다. 시스템 반응: `clear()` 후 `/login`으로 이동.
- **토큰 만료/철회 후 잔존 요청** (Phase 10 블랙리스트에 오른 토큰을 인터셉터가 계속 첨부): 인터셉터는 토큰의 유효성을 모른 채 `localStorage`에 값이 있으면 무조건 첨부한다. 증상: 서버가 401을 반환하지만 이 작업에는 **401을 가로채 자동 로그아웃/리다이렉트하는 응답 인터셉터가 없다** — 해당 호출이 실패할 뿐 자동 복구는 없다(후속 과제).

## 함정 (이번에 확인된 비직관 동작)

- 인터셉터가 zustand 스토어가 아니라 `localStorage`를 읽는다 — 스토어만 보고 "토큰 첨부 경로"를 추적하면 어디서 헤더가 붙는지 놓친다(상세 사용법은 learned §2).
- 로그아웃의 `catch {}`가 의도적으로 비어 있다 — 누락된 에러 처리가 아니라 best-effort 설계(learned §9).

## 해당 없음 사유

- 테스트 메커니즘 — 이 작업은 자동화 테스트를 추가하지 않았다(검증은 `tsc -b && vite build` 타입 통과 + 수동 시나리오). learned §8 참조.
