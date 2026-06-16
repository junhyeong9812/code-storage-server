# TECHNICAL: Phase 5 — Branch (브랜치 관리)

> 목적: 이 구현의 **diff 비종속 동작 모델**. 특정 커밋을 몰라도 유지보수자가 알아야 하는 개념·동작 원리·불변조건·실패 메커니즘. 절차/분기 다이어그램은 OVERVIEW가 소유한다 — 여기서는 그 박스들이 왜 그렇게 동작하는지를 산문으로 푼다.

## 알아야 하는 개념 (구현 전제 지식)

### 개념 1: ref(참조)와 HEAD 심볼릭 참조

① ref 는 "사람이 읽는 이름 → 커밋 해시" 매핑이다. CTS 에서는 `.cts/refs/heads/<branch>` 라는 텍스트 파일 하나가 한 브랜치이고, 내용은 그 브랜치 head 커밋의 해시 문자열(끝에 개행)이다. HEAD 는 `.cts/HEAD` 파일에 담긴 심볼릭 참조로, 내용은 `ref: refs/heads/<branch>\n` — 즉 "지금 어느 브랜치 위에 있는가"를 가리키는 포인터의 포인터다.

② Phase 5 의 branch/checkout 은 객체(blob/tree/commit)를 전혀 만들지 않는다. 브랜치 생성은 "현재 head 해시를 새 ref 파일에 복사", 브랜치 전환은 "HEAD 의 한 줄을 교체"로 환원된다. 이 개념이 없으면 "브랜치를 만들면 코드가 복제된다"는 오해로 불필요한 복사 로직을 짜게 된다.

③ 모르면 나오는 결함: HEAD 를 단순 커밋 해시로 저장(detached)해 버리면 커밋할 때 어느 브랜치를 전진시킬지 알 수 없고, ref 파일에 해시 외 잡문(공백·여러 줄)을 남기면 `read_branch` 가 head 를 잘못 해석한다.

### 개념 2: 작업 트리 / 인덱스 / 커밋 트리의 3-way 상태

① 한 시점의 저장소에는 세 가지 "파일 목록"이 공존한다 — 디스크의 실제 파일(작업 트리), 다음 커밋 후보인 스테이징(인덱스 `.cts/index`), 그리고 HEAD 커밋이 담은 스냅샷(커밋 트리). 상태(status)란 이 셋을 양방향으로 비교한 차집합이다: 인덱스↔커밋트리 = "커밋할 변경(staged)", 작업트리↔인덱스 = "스테이징 안 된 변경(not_staged)"과 "추적 안 함(untracked)".

② checkout 의 더티 검사가 status 와 정확히 같은 판정을 써야 했기 때문에, 이 3-way 비교를 `worktree::compute` 한 함수로 모으고 결과를 `StatusReport` 구조체로 표현했다.

③ 모르면 나오는 결함: 더티 검사를 status 와 따로 구현하면 두 정의가 미세하게 어긋나(예: 한쪽만 untracked 포함) "status 는 깨끗하다는데 checkout 은 거부"하거나 그 반대의 모순이 생긴다.

### 개념 3: 스냅샷 복원(checkout 모듈)과 추적 파일 제거

① `crate::checkout::checkout(repo, commit_hash)` 는 Phase 3/4 에서 이미 있던 복원기로, 커밋 트리를 재귀 순회하며 blob 을 작업 트리에 쓰고 인덱스를 그 상태로 다시 채운다. 단, **대상에 있는 파일만** 쓴다.

② 그래서 브랜치 전환 전에 이전 브랜치의 추적 파일을 따로 지워야 한다(`remove_tracked_files`). 안 그러면 옛 브랜치에만 있던 파일이 새 브랜치 작업 트리에 유령처럼 남는다.

③ 모르면 나오는 결함: 제거 단계를 빼면 `main`→`feature` 전환 시 `feature` 에 없는 `main` 전용 파일이 남아, 그 직후 status 가 그 파일을 untracked 로 보고하거나 커밋 시 의도치 않게 포함된다.

## 동작 방식

**브랜치 생성 (`create_branch`).** 이름 검증 → 중복 확인(`branch_exists`) → 현재 브랜치 이름 획득(`current_branch`, HEAD 파싱) → 현재 브랜치 head 읽기(`read_branch`) → 새 ref 파일 쓰기(`update_branch`). `update_branch` 는 쓰기 전에 부모 디렉토리를 `create_dir_all` 하므로 `feature/x` 같은 중첩 이름이 `refs/heads/feature/x` 로 자연스럽게 디렉토리화된다. head 가 `None`(아직 커밋 없음)이면 만들 기준이 없으므로 에러로 끝난다 — 브랜치는 항상 기존 커밋을 가리켜야 한다는 불변을 강제한다.

**브랜치 목록 (`list_branches`).** `refs/heads/` 디렉토리를 재귀 순회(`collect_branches`)하며 base 기준 상대 경로를 OS 경로 구분자와 무관하게 `/` 로 이어 붙여 브랜치 이름으로 만든다. 따라서 중첩 디렉토리가 그대로 `feature/x` 로 복원된다. 정렬 후 반환하고, 출력 측에서 `current_branch` 와 비교해 현재 브랜치에 `*` 를 붙인다.

**전환 (`commands/checkout::run`).** 핵심은 마지막의 세 줄 — `remove_tracked_files` → `restore::checkout(target_head)` → `refs::set_head(branch)` 의 **순서**다. 먼저 현재 인덱스(=이전 브랜치 추적 목록)를 로드해 그 파일들만 작업 트리에서 지운다(untracked 는 보존). 그다음 대상 커밋 스냅샷을 풀어 작업 트리와 인덱스를 대상 상태로 덮어쓴다. 마지막에야 HEAD 를 옮긴다. set_head 를 맨 끝에 두는 이유: 파일 시스템 작업(제거/복원) 도중 실패해도 HEAD 는 여전히 이전 브랜치를 가리켜, "어느 브랜치 위인지"의 권위 있는 표식이 거짓이 되지 않게 한다.

**더티 검사.** 전환 전에 `worktree::compute(repo)` 로 `StatusReport` 를 만들고 `has_uncommitted()`(staged 또는 not_staged 가 비지 않음)가 참이면 거부한다. untracked 는 일부러 제외한다 — untracked 파일은 어느 커밋에도 속하지 않으므로 복원이 덮어쓰지 않고 그대로 남아 손실 위험이 없기 때문이다.

## 불변조건 / 계약

- **HEAD 는 항상 `ref: refs/heads/<branch>\n` 형식이다.** 깨지면 `current_branch` 가 `HEAD 형식을 해석할 수 없습니다` 로 bail 하고, 그 위에 선 branch/checkout/status 가 모두 멈춘다.
- **ref 파일(`refs/heads/<branch>`)의 내용은 커밋 해시 한 줄(또는 빈 내용)이다.** `read_branch` 는 trim 후 빈 문자열을 `None` 으로 해석한다 — 빈 ≠ 없음이 아니라 "head 미정"으로 동일 취급.
- **브랜치는 존재하는 커밋만 가리킨다.** `create_branch` 는 현재 head 가 없으면 거부한다. 따라서 ref 파일이 생겼다면 그 안의 해시는 반드시 실존 커밋이다.
- **checkout 성공 후: 작업 트리 = 인덱스 = 대상 커밋 트리, 그리고 HEAD = 대상 브랜치.** 더티 검사가 이 등식의 출발점(전환 전 깨끗)을 보장한다.
- **checkout 더티 판정 = `has_uncommitted()`(untracked 제외).** status 의 `is_clean()` 과 정의가 다르다(is_clean 은 untracked 도 본다). 둘을 혼동하면 안 된다.

## 상태와 소유권

- **현재 브랜치의 source of truth = `.cts/HEAD` 파일** 한 곳. 갱신자는 `refs::set_head`(전환 시)와 `Repo::init`(최초 `main` 설정). 다른 곳에서 현재 브랜치를 캐시하지 않고 매번 `current_branch` 로 파일에서 읽는다.
- **브랜치 head 의 source of truth = 각 `refs/heads/<branch>` 파일.** 갱신자는 `update_branch`(브랜치 생성·커밋 전진). Phase 5 는 생성만 한다.
- **StatusReport 는 파생값이며 저장하지 않는다.** 매 호출 `compute` 가 작업 트리를 디스크에서 다시 해싱(`Blob::new(...).hash()`)해 계산한다 — 캐시가 없어 항상 최신이지만 파일이 많으면 매번 전수 해싱하는 비용이 있다.

## 외부 경계와 의존성

- 외부 네트워크/서버 경계 없음 — Phase 5 는 순수 로컬 작업이다. 서버는 Phase 4 에서 이미 브랜치별 저장(push/pull 의 branch 파라미터)을 지원하므로 이번 변경은 원격을 건드리지 않는다.
- 신뢰 경계는 **로컬 파일 시스템**뿐이다. `.cts/` 하위 파일과 작업 트리를 `std::fs` 로 직접 읽고 쓴다. 실패 모드는 아래 절 참조.

## 실패 모드 메커니즘

- **이름 부적합 / 중복 / 커밋 없음 (생성 시):** 원인은 사용자 입력. 증상은 즉시 `bail!` 로 한국어 에러 메시지. 부작용 없음(ref 파일 생성 전에 검증). `create_branch` 가 검증→중복→head 순으로 단락 평가하므로 첫 실패에서 멈춘다.
- **대상 브랜치 없음 (전환 시):** `-b` 없이 미존재 브랜치를 checkout 하면 `branch_exists` 가 false → `브랜치가 없습니다 ... (-b 옵션)`. 작업 트리 무변경.
- **더티 트리 (전환 시):** `has_uncommitted()` 참 → 전환 전에 거부. 핵심 안전장치 — 복원이 시작되기 전이라 작업 트리는 손대지 않는다.
- **복원 도중 파일 시스템 오류:** `remove_tracked_files` 는 `remove_file(..).ok()` 로 개별 실패를 무시(권한·이미 없음 등)하지만, 이어지는 `restore::checkout` 의 쓰기 실패는 `?` 로 전파된다. 이때 HEAD 는 아직 set_head 전이라 이전 브랜치를 가리킨 채 남는다 — 작업 트리는 일부만 복원된 중간 상태일 수 있다(원자적 전환은 아님; 한계).
- **빈 디렉토리 정리 경합:** `remove_tracked_files` 는 파일 삭제 후 빈 상위 디렉토리를 위로 올라가며 `remove_dir` 시도하되, 루트 도달 또는 실패(비어있지 않음/권한) 시 break — 다른 파일이 남은 디렉토리는 건드리지 않는다.

## 함정 (이번에 확인된 비직관 동작)

- **checkout 의 더티 검사는 untracked 를 손실 위험으로 보지 않는다.** `has_uncommitted()` 가 staged/not_staged 만 본다는 사실은 직관과 어긋날 수 있다(추적 안 한 새 파일이 있어도 전환이 허용됨). 근거: untracked 는 복원이 덮어쓰지 않으므로 안전. 관련 함수 상세는 learned.
- **status 의 `is_clean()` 과 checkout 의 `has_uncommitted()` 는 반대말이 아니다.** is_clean 은 untracked 까지 포함(셋 다 비어야 깨끗), has_uncommitted 는 untracked 제외. 같은 `StatusReport` 의 서로 다른 질의다.
- **브랜치 이름의 `/` 는 디렉토리가 된다.** `feature/x` 는 `refs/heads/feature/x` 파일이라 `feature` 라는 이름의 파일과 `feature/x` 가 공존할 수 없다(git 과 동일한 D/F 충돌 가능성). 검증은 `..`·선행/후행 `/` 만 막고 이 충돌까지는 막지 않는다(한계).

## 해당 없음 사유

- 외부 경계 절은 "신뢰 경계 = 로컬 FS"로 채웠으므로 별도 없음 처리 불필요. 네트워크/DB/큐/브라우저 경계는 이 phase 범위 밖(로컬 전용)이라 다루지 않음.
