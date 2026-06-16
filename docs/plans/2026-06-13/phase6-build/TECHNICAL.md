# TECHNICAL: Phase 6 — Build (CI/CD)

> 목적: build 도메인의 **diff 비종속 동작 모델**. 트리 복원·셸 실행·상태 전이가 런타임에 왜 그렇게 움직이는지, 어떤 불변조건과 실패 모드가 있는지 해설한다. 절차 다이어그램은 OVERVIEW 가 소유한다 — 여기는 그 박스들의 "왜"만 다룬다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: 헥사고날(포트-어댑터) 아키텍처
① 도메인이 인터페이스(포트)를 정의하고, 인프라가 그 구현(어댑터)을 제공하며, 상위 레이어는 포트만 의존하는 구조. ② 이 작업에서 빌드 "기록"(`BuildRepository`)과 빌드 "실행"(`BuildRunner`)을 각각 포트로 두어, PostgreSQL/셸이라는 구체 기술을 도메인·유스케이스에서 분리했다. ③ 모르면 유스케이스가 sqlx·tokio::process 에 직접 묶여 러너를 Docker 로 교체할 수 없고 테스트도 어려워진다.

### 개념 2: 콘텐츠 주소 객체 그래프(트리/커밋)
① git 류 저장소처럼 commit 이 tree 를, tree 가 blob/하위 tree 를 해시로 참조하는 그래프. ② 빌드는 "체크아웃"이 필요한데, 워킹 카피가 따로 없으므로 commit 의 `tree_hash` 에서 시작해 그래프를 따라 파일을 디스크에 다시 써야(materialize) 한다. ③ 모르면 빌드 명령이 빈 디렉토리에서 돌아 항상 실패한다.

### 개념 3: BuildStatus 생명주기 상태 머신
① 하나의 빌드가 거치는 상태 집합과 전이 규칙. ② pending(생성)→running(실행 시작)→success/failed(종료) 의 단방향 진행을 모델링하고, 종료 여부를 `is_terminal()` 로 판정한다. ③ 모르면 종료된 빌드를 다시 실행 중으로 되돌리거나, DB 문자열과 enum 이 어긋나 상태 표시가 깨진다.

## 동작 방식

**트리 복원(materialize).** `ShellBuildRunner::materialize` 는 `objects.get_tree_entries(repo, tree_hash)` 로 한 디렉토리의 엔트리를 받아, 엔트리의 `object_type` 이 `"blob"` 이면 `blobs.get` 으로 내용을 받아 파일로 쓰고, `"tree"` 면 디렉토리를 만든 뒤 자기 자신을 그 하위 해시로 재귀 호출한다. Rust 의 `async fn` 은 자기 자신을 직접 재귀 호출할 수 없으므로(무한 크기 future) 반환 타입을 `Pin<Box<dyn Future<Output=...> + Send + 'a>>` 로 명시하고 `Box::pin(async move { ... })` 로 감싸 힙에 박스화한다. 이렇게 해야 future 크기가 컴파일 타임에 고정된다.

**빌드 명령 결정과 실행.** 트리 복원 후, 요청 `command` 가 있으면 그대로, 없고 체크아웃 루트에 `cts.build.sh`(상수 `BUILD_SCRIPT`)가 파일로 존재하면 `"sh cts.build.sh"`, 둘 다 아니면 명령 없음으로 처리한다. 명령이 정해지면 `tokio::process::Command::new("sh").arg("-c").arg(cmd).current_dir(workdir).output()` 로 셸을 띄워 stdout/stderr 를 통째로 캡처한다. `cwd` 를 workdir 로 고정하기 때문에 빌드 스크립트가 상대 경로로 트리 파일을 참조할 수 있다. 성공 여부는 프로세스 종료 코드(`output.status.success()`)로만 판정한다 — stderr 출력 유무와 무관하다.

**상태 전이 기록.** 전이는 유스케이스 `run_build` 가 순서대로 명령한다: `create`(pending) → `mark_running`(running, started_at) → 실행 → `mark_finished`(success|failed, finished_at, log_path). 각 전이는 별도 UPDATE 문이라 원자적 트랜잭션이 아니다(아래 불변조건·실패 모드 참조).

## 불변조건 / 계약

- **BuildStatus 문자열 매핑은 `as_str`/`from_db` 가 전단사여야 한다.** 한쪽에 상태를 추가하고 다른 쪽을 빠뜨리면 `from_db` 가 `AppError::Storage("알 수 없는 빌드 상태")` 로 실패한다 — `db_roundtrip` 테스트가 이를 강제한다.
- **빌드는 반드시 서버에 이미 push 된 커밋을 가리켜야 한다.** `create` 가 `commits` 에서 (repository_id, hash) 로 commit_id 를 찾지 못하면 행을 만들지 않고 `InvalidInput` 으로 거른다 — DB FK(commit_id NOT NULL 가정) 위반을 미리 막는다.
- **로그 파일 경로는 build_id 로 유일하다** — `<logs_dir>/<build_id>.log`. build_id 가 곧 INSERT 된 PK 이므로 로그 충돌이 없다.
- **workdir 는 실행 전후로 정리된다** — 시작 전 `remove_dir_all`(실패 무시) + 종료 후 `remove_dir_all`. 정리는 best-effort 라 실패해도 빌드 결과에 영향을 주지 않는다.

## 상태와 소유권

**BuildStatus 상태 머신 (source of truth = DB `builds.status`)**

```
   create()              mark_running()           mark_finished(Success)
 ──────────▶ [pending] ───────────────▶ [running] ──────────────────▶ [success]  ← terminal
                                            │      
                                            └──────────────────────▶ [failed]   ← terminal
                                                mark_finished(Failed)
```

- 전이 규칙: pending→running→{success|failed}. success/failed 는 `is_terminal()==true` 로 더 이상 전이가 정의되지 않는다.
- `Build` 엔티티는 불변 스냅샷이다 — 필드는 private, 접근자(getter)만 노출하고, 상태 변경 메서드가 없다. 갱신은 엔티티가 아니라 `BuildRepository` 의 `mark_*` 가 DB 에 직접 수행한다(엔티티는 읽기 모델, DB 가 권위).
- `commit_hash` 는 표시용 파생값이다 — 저장 권위는 commit_id(UUID FK)이고, 조회 시 `JOIN commits` 로 해시를 계산해 채운다. 엔티티에는 둘 중 해시만 들고 있다.
- started_at/finished_at/log_path 는 전이가 진행되며 채워지는 `Option` — pending 시 모두 None.

## 외부 경계와 의존성

- **PostgreSQL (`builds`, `commits` 테이블)** — sqlx `PgPool` 런타임 쿼리. 신뢰 수준: 스키마는 기존 마이그레이션 소유, 본 작업은 행만 읽고 쓴다. 실패 모드: 연결/쿼리 오류는 `db_err` 로 전부 `AppError::Storage` 로 평탄화 → HTTP 500.
- **셸 프로세스 (`sh -c`)** — 빌드 명령을 **샌드박스 없이 서버 프로세스 권한으로** 실행한다. 신뢰 경계 = 서버 자신 즉, 요청 `command` 는 임의 코드 실행과 같다(현재 인증 없음, Phase 6 범위 밖). 실패 모드: 명령 종료 코드 ≠0 → failed(에러 아님, 정상 결과). 프로세스 spawn 자체 실패(`sh` 없음 등) → `AppError::Storage`.
- **파일시스템 (`STORAGE_PATH/builds/`)** — `work/<build_id>`(체크아웃 임시 트리), `logs/<build_id>.log`(로그). 디렉토리는 실행 시 `create_dir_all` 로 보장. 실패 모드: 쓰기 실패 → `AppError::Storage`.

## 실패 모드 메커니즘

- **미존재 커밋** — 원인: 클라이언트가 push 안 된 해시 요청. 증상: `create` 의 commit_id 조회가 None. 반응: `InvalidInput` → HTTP 400, 빌드 행 미생성(부분 상태 없음).
- **빌드 명령 실패(exit ≠0)** — 원인: 스크립트/명령 자체 실패. 증상: `output.status.success()==false`. 반응: 이는 정상 흐름 — `mark_finished(Failed)` 로 기록하고 로그에 `--- exit: ... ---` 까지 남긴 뒤 201 로 failed 빌드를 반환한다(HTTP 에러 아님).
- **빌드 명령 없음** — 원인: command 미지정 + cts.build.sh 부재. 증상: resolved_command=None. 반응: 실행 없이 안내 메시지를 로그에 쓰고 success=false → failed.
- **전이 중 크래시(원자성 없음)** — 원인: create 와 mark_running/mark_finished 가 별도 UPDATE 라, 중간에 프로세스가 죽으면 빌드가 running 에 멈춘 채 영구히 남는다(좀비). 증상: 종료되지 않는 running 빌드. 현재 복구·타임아웃 로직 없음(데모 한계, changelog J-4).
- **장기 빌드 HTTP 타임아웃** — 원인: run_build 가 완료까지 인라인 await. 증상: 빌드가 길면 클라이언트/프록시가 먼저 끊긴다. 반응: 서버 측 빌드는 계속 진행되어 DB 에는 결과가 남지만 클라이언트는 응답을 못 받는다.

## 함정 (이번에 확인된 비직관 동작)

- **async 재귀는 그냥 안 된다** — `materialize` 를 평범한 `async fn` 으로 자기 호출하면 "recursion in an async fn requires boxing" 컴파일 에러. `Pin<Box<dyn Future + Send + 'a>>` 수동 반환이 필요(learned §2 참조).
- **failed 빌드는 HTTP 200/201 이다** — 빌드 실패와 요청 실패는 다른 층위. 빌드 자체가 exit 1 이어도 API 는 "빌드를 정상적으로 실행해서 failed 라는 결과를 얻음"이므로 201 Created 를 돌려준다. 400/404/500 은 빌드 결과가 아니라 요청·시스템 오류 전용.
- **로그가 비어 있어도 에러가 아니다** — `get_build_log` 는 log_path 가 None(아직 안 끝난 빌드 등)이면 빈 문자열을 반환한다.

## 해당 없음 사유

- 동시성 제어/락 — 해당 없음: 같은 커밋 중복 빌드를 막는 잠금이 없다(각 요청이 독립 build_id). 의도된 단순화.
