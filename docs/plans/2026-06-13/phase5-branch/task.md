# Phase 5 — Branch (브랜치 관리)

## 범위
로컬 브랜치 생성/목록/전환. 서버는 Phase 4에서 이미 브랜치별 저장을
지원하므로(push/pull 의 branch 파라미터), Phase 5는 주로 CLI 로컬 작업.

## 구현
- `refs.rs`: set_head / branch_exists / list_branches(중첩 포함)
- `worktree.rs` (신규): status 의 3-way 비교 로직을 compute() 로 추출.
  - StatusReport: is_clean() / has_uncommitted()
  - status 와 checkout(더티 검사)이 공용으로 사용
- `cts branch [name]`: 목록(현재 *) / 현재 커밋에서 새 브랜치 생성
- `cts checkout [-b] <branch>`: 전환
  - 커밋되지 않은 변경 있으면 거부(데이터 손실 방지)
  - 이전 추적 파일 제거 → 대상 커밋 스냅샷 복원 → HEAD 갱신

## 커밋 메모
- main.rs(모듈선언 + clap enum + dispatch)가 여러 변경의 합류점이라,
  중간 커밋이 컴파일되지 않음. → 브랜치 관리는 1개 응집 커밋으로 묶음.

## 검증
- ✅ `cargo test` 전체 green: cli 2 + core 25 + server 7 + doctest 18 = 52.
- ✅ 로컬: branch 생성/목록(*표시), checkout 전환 시 작업트리 동기화
  (feature.txt 출현/소멸), 브랜치별 log 상이, dirty checkout 거부, -b 생성+전환.
- ✅ 멀티 브랜치 서버 연동(E2E):
  - main/feature 각각 push → 서버 branches 2개(각 head 일치)
  - clone(main) 후 로컬 feature 생성 → pull → feature.txt+main.txt, 커밋 2개

## 한계 / 다음
- fast-forward/머지 없음. 브랜치 삭제(`branch -d`) 미구현.
- 원격 브랜치 자동 추적/목록 미구현(현재 브랜치만 pull).
- Phase 6: Build (CI/CD) — builds 테이블/도메인 활용, 커밋 빌드 실행.
</content>
