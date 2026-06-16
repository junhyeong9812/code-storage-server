# OVERVIEW: Phase 5 — Branch (브랜치 관리)

> 목적: 이 구현의 **추상 진입점** — `cts branch` / `cts checkout` 두 명령이 무엇을 하고 어떤 순서·분기로 도는지 한눈에 본다. 메커니즘의 "왜"는 TECHNICAL, 선택의 "왜"는 changelog(J/M/G), 요소 카탈로그는 learned.

## 주요 포인트 (3~7)

- **브랜치는 파일 하나다.** `cts branch <name>` 은 현재 브랜치 head 커밋 해시를 `.cts/refs/heads/<name>` 텍스트 파일에 쓰는 것이 전부다 — 새 객체·스냅샷 복사는 없다. 까다로운 곳은 중첩 브랜치 이름(`feature/x`)의 디렉토리 매핑. → 메커니즘 `TECHNICAL §동작 방식`, 선택 이유 `changelog J-2`.

- **현재 브랜치는 HEAD 한 줄로 표현된다.** `cts checkout` 의 본질은 `.cts/HEAD` 에 `ref: refs/heads/<branch>` 한 줄을 다시 쓰는 것(`refs::set_head`). 작업 트리 갱신은 그 부산물. → 선택 이유 `changelog J-4`, 불변조건 `TECHNICAL §불변조건`.

- **`branch` 목록과 `branch <name>` 생성은 같은 명령의 두 분기다.** 인자 `Option<String>` 의 None/Some 으로 갈린다. 현재 브랜치에 `*` 표시. 까다로운 곳은 이름 검증(슬래시·`..` 거부). → 선택 이유 `changelog J-3`.

- **checkout 은 데이터 손실을 거부로 막는다.** 커밋되지 않은 변경(staged/not_staged)이 있으면 전환 자체를 중단한다. 이 "더티 검사"는 `status` 와 **같은 비교 엔진**(`worktree::compute`)을 쓴다. 까다로운 곳은 untracked 를 손실 위험에서 제외하는 경계. → 메커니즘 `TECHNICAL §동작 방식`, 추출 결정 `changelog J-1`.

- **status 의 3-way 비교 로직이 `worktree.rs` 로 추출됐다.** status 출력과 checkout 더티 검사가 같은 `StatusReport` 를 공유한다. status.rs 는 출력만 남고 비교 로직은 통째로 이동(라인 포맷은 동일하나 정렬 키가 "포맷 문자열 전체"→"경로"로 바뀐 미묘한 동작 변화). → 선택·정렬 변화 `changelog J-5`, 카탈로그 `learned §5`.

- **전환 시 작업 트리 동기화는 "제거 → 복원 → HEAD 갱신" 순서다.** 이전 브랜치의 추적 파일을 지운 뒤(`remove_tracked_files`) 대상 커밋 스냅샷을 풀고(`crate::checkout::checkout`, Phase 3/4 의 복원기 재사용) 마지막에 HEAD 를 옮긴다. 까다로운 곳은 순서(중간 실패 시 HEAD 는 아직 옛 브랜치). → 실패 모드 `TECHNICAL §실패 모드 메커니즘`.

## 워크플로우 (절차 + 분기)

```
cts branch [name]
  │
  ├─ name 없음 ─▶ [list] refs::list_branches + current_branch
  │                 └─▶ 각 브랜치 출력, 현재 브랜치에 "*"  ──▶ (목록 출력)
  │
  └─ name 있음 ─▶ [create_branch]
                    │
                    ├─ 이름 부적합? ──예──▶ (에러: 올바르지 않은 이름)
                    ├─ 이미 존재?   ──예──▶ (에러: 이미 존재)
                    ├─ 현재 브랜치에 커밋 없음? ──예──▶ (에러: commit 먼저)
                    └─ 정상 ──▶ refs::update_branch(name, head) ──▶ (브랜치 생성)


cts checkout [-b] <branch>
  │
  ├─ -b 지정? ──예──▶ branch::create_branch (위 create 분기와 동일 검증)
  │
  ▼
[브랜치 존재 확인] ── 없음 ──▶ (에러: -b 로 생성하세요)
  │ 있음
  ▼
[현재 == 대상?] ── 예 ──▶ (이미 그 브랜치임 / no-op)
  │ 아니오
  ▼
[worktree::compute] ── has_uncommitted()? ── 예 ──▶ (에러: commit 먼저 / 전환 거부)
  │ 아니오(깨끗)
  ▼
[대상 head 읽기] ── 커밋 없음 ──▶ (에러: 대상에 커밋 없음)
  │ 있음
  ▼
[remove_tracked_files] ──▶ [crate::checkout::checkout(대상 head)] ──▶ [refs::set_head(branch)]
  │
  ▼
(전환 완료 — 작업 트리 = 대상 스냅샷, HEAD = 대상 브랜치)
```

> 각 박스가 **왜 그렇게 동작하는가**(예: 왜 untracked 는 더티에서 빼는가, 왜 set_head 가 마지막인가)는 TECHNICAL 메커니즘 산문에서 해설한다.

## 딥다이브 인덱스

| 알고 싶은 것 | 문서·절 |
|---|---|
| 왜 그렇게 동작하나 (ref 모델·불변조건·전환 순서·실패) | TECHNICAL |
| 이번에 왜 그렇게 바꿨나 (선택·대안·근거) | changelog (J-1~J-7, M, 셀프체크) |
| 무슨 요소를 어떻게 썼나 (refs API·clap·std::fs·anyhow) | learned |
| 리뷰에서 무엇이 지적되고 어떻게 해소됐나 | review-log (없음 — 단독 구현, 사후 기록) |
