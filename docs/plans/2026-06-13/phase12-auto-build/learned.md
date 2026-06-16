# 학습 기록 (Learned)

> 작성일: 2026-06-13
> 관련 산출물: docs/plans/2026-06-13/phase12-auto-build/task.md
> 작업 요약: push 성공 후 커밋 루트에 `cts.build.sh` 가 있으면 `tokio::spawn` 으로 백그라운드 빌드를 자동 트리거 (Phase 4 push ↔ Phase 6 build 배선).

> 이 문서는 changelog J-1 의 의사결정을 전제로, **재사용한 요소·패턴**을 카탈로그화한다. 스니펫은 스냅샷 tree 에서 직접 복사.

---

## 1. 사용된 라이브러리

| 라이브러리 | 버전 | 용도 | 왜 선택했는가 |
|-----------|------|------|-------------|
| tokio | (워크스페이스 핀, runtime) | `tokio::spawn` 으로 빌드 실행을 detached 태스크로 분리 | axum 런타임이 이미 tokio. push 응답을 블록하지 않고 빌드를 백그라운드로 보내는 표준 방법 |
| tracing | (워크스페이스 핀) | 자동 빌드 실패를 `warn!` 으로 기록 | 에러를 흡수하되 흔적은 남기는 구조적 로깅 |
| axum | (워크스페이스 핀) | `push_handler` 의 `State`/`Json` 추출, 핸들러 시그니처 | 기존 핸들러 층 그대로 |
| uuid | (워크스페이스 핀) | `repo_id: Uuid` 식별자 | 기존 `Path<Uuid>` 추출과 일관 |

> 이 Phase 는 새 의존성을 추가하지 않는다 — 전부 기존 server 크레이트가 쓰던 것을 재사용.

---

## 2. 핵심 함수 / 메서드

### tokio

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `tokio::spawn` | `fn spawn<F>(future: F) -> JoinHandle<F::Output> where F: Future + Send + 'static, F::Output: Send + 'static` | future 를 새 태스크로 스케줄, JoinHandle 즉시 반환(여기선 버림 → detached) | handlers/mod.rs:141 |

### build 유스케이스 (Phase 6 재사용)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `run_build` | `async fn run_build(builds: &dyn BuildRepository, runner: &dyn BuildRunner, repository_id: Id, commit_hash: &str, command: Option<&str>) -> Result<Build, AppError>` | pending 생성 → running → runner.run → 종료 상태 기록 후 Build 반환 | handlers/mod.rs:142 (자동), build/api/handlers/mod.rs:27 (수동) |

### ObjectRepository 포트 (Phase 4 영역 — 조회만 재사용)

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `get_commit` | `async fn get_commit(&self, repository_id: RepositoryId, hash: &str) -> Result<Option<CommitRecord>, AppError>` | 커밋 해시로 `CommitRecord`(→ `tree_hash`) 조회 | handlers/mod.rs:122 |
| `get_tree_entries` | `async fn get_tree_entries(&self, repository_id: RepositoryId, hash: &str) -> Result<Vec<TreeEntryRecord>, AppError>` | 트리 해시로 엔트리 목록(`name`, `object_type`, ...) 조회 | handlers/mod.rs:124 |

### Arc

| 함수/메서드 | 시그니처 | 역할 | 사용 위치 |
|------------|---------|------|----------|
| `Arc::clone` (`.clone()`) | `fn clone(&self) -> Arc<T>` | 참조 카운트 +1, 어댑터 인스턴스 공유한 채 소유권 복제 | handlers/mod.rs:138-139 |
| `Arc::as_ref` (`.as_ref()`) | `fn as_ref(&self) -> &T` | `Arc<dyn Trait>` → `&dyn Trait` 로 빌려 `run_build` 인자에 전달 | handlers/mod.rs:142 |

**사용 예시:**
```rust
    let builds = state.builds.clone();
    let runner = state.build_runner.clone();
    let commit = commit_hash.to_string();
    tokio::spawn(async move {
        if let Err(e) = run_build(builds.as_ref(), runner.as_ref(), repo_id, &commit, None).await {
            tracing::warn!(error = %e, "자동 빌드 실패");
        }
    });
```
- 출처: `crates/server/src/repository/api/handlers/mod.rs:138-145`

**코드 설명:**
> `state.builds.clone()` / `state.build_runner.clone()` — `Arc<dyn ...>` 의 카운트만 늘려 포트 소유권을 클로저로 넘긴다(`&state` 를 빌리지 않는 이유: spawn 은 `'static` 요구).
> `commit_hash.to_string()` — `&str` 을 소유 `String` 으로 만들어 클로저가 호출 스택보다 오래 살 수 있게 한다.
> `tokio::spawn(async move { ... })` — `move` 로 캡처한 future 를 detached 태스크로 올린다. JoinHandle 을 받지 않아 push 핸들러는 결과를 기다리지 않는다.
> `run_build(builds.as_ref(), runner.as_ref(), repo_id, &commit, None)` — Phase 6 유스케이스. `None` 은 "기본 명령(저장소 루트 `cts.build.sh`)"을 의미.
> `tracing::warn!(error = %e, "자동 빌드 실패")` — 실패를 전파하지 않고 구조적 로그로만 남겨 실패를 격리.

---

## 3. 어노테이션 / 데코레이터

| 어노테이션/데코레이터 | 소속 | 역할 | 적용 대상 |
|--------------------|------|------|----------|
| `#[async_trait]` | async-trait | 트레이트에 `async fn` 허용(반환 future 박싱) | `ObjectRepository` 포트 정의 (재사용만, 이번 diff 에서 정의 변경 없음) |

**동작 원리:**
`async_trait` 는 `async fn` 을 `Pin<Box<dyn Future + Send>>` 반환 메서드로 데슈가링한다. 이 Phase 는 포트를 새로 정의하지 않고 기존 `ObjectRepository`/`BuildRepository` 트레이트 객체(`Arc<dyn ...>`)를 호출만 한다.

> 참고(함정): 이 프로젝트는 `core` 크레이트가 std `core` 를 가려 `async_trait` 매크로가 깨지는 이슈가 있어 별칭 `cts_core` 를 쓴다(MEMORY). **이번 diff 는 `core`/`cts_core` 를 import 하지 않으므로 해당 함정과 무관** — 추가 import 는 모두 같은 server 크레이트 내부 경로.

---

## 4. 수정 전/후 코드 비교

### 파일명: `crates/server/src/repository/api/handlers/mod.rs`

**수정 전 (push_handler):**
```rust
pub async fn push_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: AuthUser,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    require_write(&state, id, &auth).await?;
    let response = push(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        request,
    )
    .await?;
    Ok(Json(response))
}
```

**수정 후 (push_handler + maybe_auto_build):**
```rust
pub async fn push_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    auth: AuthUser,
    Json(request): Json<PushRequest>,
) -> Result<Json<PushResponse>, ApiError> {
    require_write(&state, id, &auth).await?;
    let response = push(
        state.objects.as_ref(),
        state.blobs.as_ref(),
        RepositoryId::from_uuid(id),
        request,
    )
    .await?;
    // 자동 빌드: 커밋에 cts.build.sh 가 있으면 백그라운드로 실행
    maybe_auto_build(&state, id, &response.commit_hash).await;
    Ok(Json(response))
}
```
(신규 `maybe_auto_build` 함수 전문은 changelog J-1 스니펫 참조 — 중복 기재 금지.)

**변경 이유:** push 성공 직후에만 빌드를 트리거하고(브랜치 head 확정 후), 빌드는 백그라운드로 분리해 push 응답을 지연시키지 않기 위해.

**변경된 함수/메서드 설명:**
| 함수/메서드 | 변경 내용 | 이유 |
|------------|----------|------|
| `push_handler` | `?` 통과(성공) 후 `maybe_auto_build(...).await` 1줄 삽입 | push 성공이라는 불변조건 위에서만 트리거 |
| `maybe_auto_build` (신규) | 스크립트 탐지(동기) + 빌드 실행(spawn) | 탐지/실행 동기성 분리 · 실패 격리 |

---

## 5. 동작 구조

### 실행 흐름
```
POST /push
  → push_handler
     → require_write (권한)
     → push() : 객체 업로드 + 브랜치 head 갱신 → PushResponse{commit_hash}
     → maybe_auto_build(&state, id, commit_hash)   [동기 await]
        → objects.get_commit(repo, hash)            → CommitRecord{tree_hash}
        → objects.get_tree_entries(repo, tree_hash) → Vec<TreeEntryRecord>
        → any(name=="cts.build.sh" && type=="blob") → has_script
        → has_script ? tokio::spawn(run_build(..., None)) : return
     ← Ok(Json(response))    [HTTP 응답, 빌드 대기 안 함]
  (백그라운드) run_build → create → mark_running → runner.run → mark_finished
                         └─ Err → tracing::warn!
```

### 컴포넌트별 역할
| 컴포넌트 | 파일 | 역할 | 호출하는 메서드 |
|----------|------|------|---------------|
| `push_handler` | repository/api/handlers/mod.rs | HTTP 진입·권한·push 호출·트리거 배선 | `push`, `maybe_auto_build` |
| `maybe_auto_build` | repository/api/handlers/mod.rs | 트리거 게이트 판정 + spawn | `get_commit`, `get_tree_entries`, `tokio::spawn`, `run_build` |
| `ObjectRepository`(어댑터) | repository/domain/ports/object_repository.rs | 커밋/트리 조회 | `get_commit`, `get_tree_entries` |
| `run_build` | build/application/use_cases/mod.rs | 빌드 생성·실행·상태 기록 | `BuildRepository`/`BuildRunner` 메서드 |

### 데이터 흐름
```
PushResponse.commit_hash (&str)
  → get_commit → CommitRecord.tree_hash (String)
  → get_tree_entries → Vec<TreeEntryRecord>{ name, object_type, ... }
  → filter(name=="cts.build.sh" && object_type=="blob") → bool has_script
  → (Arc<dyn BuildRepository>, Arc<dyn BuildRunner>, repo_id: Uuid, commit: String)
  → run_build → Build (success/failed)
```

---

## 6. 디자인 패턴

| 패턴 | 적용 위치 | 왜 사용했는가 | 구조 |
|------|----------|-------------|------|
| Fire-and-forget (detached task) | `tokio::spawn` 빌드 실행 | push 응답을 블록하지 않고 실패를 격리 | spawn 후 JoinHandle 버림 |
| Ports & Adapters (헥사고날) | `Arc<dyn BuildRepository/Runner/ObjectRepository>` | 도메인 비결합, 어댑터 교체 가능 | 핸들러가 포트 트레이트 객체만 의존 |
| Guard clause | `if !has_script { return; }` | 비대상 push 조기 탈출(노이즈 0) | 조건 불충족 시 early return |

**패턴 상세:**

### Fire-and-forget (detached task)
- **의도**: 응답 경로에서 분리해야 하는 부수 작업(빌드)을, 호출자가 완료를 기다리지 않고 실행.
- **구조**: `tokio::spawn` 이 `'static`+`Send` future 를 스케줄러에 올림. 캡처값은 미리 소유화(Arc clone / String).
- **이 프로젝트에서의 적용**:
```rust
    let builds = state.builds.clone();
    let runner = state.build_runner.clone();
    let commit = commit_hash.to_string();
    tokio::spawn(async move {
        if let Err(e) = run_build(builds.as_ref(), runner.as_ref(), repo_id, &commit, None).await {
            tracing::warn!(error = %e, "자동 빌드 실패");
        }
    });
```
- 출처: `crates/server/src/repository/api/handlers/mod.rs:138-145`

---

## 7. 설정 / 컨벤션

| 항목 | 값 | 이유 |
|------|---|------|
| 트리거 파일명 | `cts.build.sh` | 노이즈 방지 게이트. 루트 트리 blob 일 때만 트리거 |
| 빌드 명령 | `None` (run_build 인자) | 러너가 저장소 루트 `cts.build.sh` 를 기본 실행 |
| 빌드 대상 커밋 | `response.commit_hash` | 방금 push 된 head |

---

## 8. 테스트에서 사용된 것들

이번 diff 는 핸들러 함수만 변경했고 스냅샷에 신규 단위 테스트 파일은 없다. 검증은 ① 기존 스위트 회귀(`cargo test` 57 green) ② 수동 E2E(스크립트 있는/없는 push 의 빌드 건수·push 지연)로 했다(출처: task.md §결과). 따라서 신규 테스트 프레임워크·픽스처·mock 추가 없음 — **해당 없음**.

---

## 9. 새로 알게 된 것

- "백그라운드 빌드"가 push 경로를 완전히 0지연으로 만들지는 않는다. 빌드 *실행*만 spawn 으로 빠지고, 트리거 *탐지*(`get_commit`+`get_tree_entries`)는 push 응답 전에 동기로 await 된다. "지연 없음"의 정확한 의미는 "빌드 완료를 기다리지 않음".
- `tokio::spawn` 의 `'static` 제약 때문에 `&AppState` 를 캡처할 수 없고, 필요한 포트만 `Arc::clone` 하고 `&str` 을 `String` 으로 소유화해 `move` 로 넘기는 게 정석.
- 반환형을 `Result` 가 아니라 `()` 로 두는 것 자체가 "이 실패는 호출자(push)로 전파하지 않는다"는 설계 계약을 코드로 표현한 것.
- 자동/수동 빌드가 같은 `run_build(..., None)` 을 공유 — 트리거 경로만 다르고 빌드 의미론은 한 곳에 모인다(DRY).

---

## 10. 더 공부할 것

| 주제 | 왜 공부해야 하는가 | 참고 자료 |
|------|-----------------|----------|
| detached task 와 graceful shutdown | spawn 한 빌드는 join 되지 않아 서버 종료 시 잘릴 수 있음 | tokio `JoinHandle`, runtime shutdown |
| 빌드 큐/동시성 제한 | 현재 push 마다 무제한 spawn — 동시 빌드 폭주 가능(task.md 한계) | semaphore / 작업 큐 패턴 |
| 빌드 결과 알림 | 현재 폴링만 — 실패가 표면화 안 됨(task.md 후속) | SSE/webhook |
| async-trait + `cts_core` 별칭 함정 | 이번엔 무관했지만 포트 추가 시 재현 가능 | MEMORY: core-crate-name-shadows-std.md |

---
