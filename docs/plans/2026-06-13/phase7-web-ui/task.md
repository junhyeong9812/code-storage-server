# Phase 7 — Web UI (코드 브라우저)

## 범위 / 결정
- **코드 브라우저**: 저장소 목록 → 브랜치/커밋 → 트리/파일 브라우징 + 빌드.
- **Vite 개발서버 + CORS** (서버가 정적 서빙하지 않음).
- 의미 있는 UI 를 위해 **서버에 읽기 전용 엔드포인트** 추가 필요했음.

## 구현 (커밋 단위)
1. `feat(server): 브라우징 읽기 엔드포인트 + CORS`
   - ObjectRepository.list_branches
   - use_cases/browse: list_branches / list_commits / browse_tree / read_blob
   - DTO: BranchDto / CommitSummary / TreeEntryDto / BlobContentDto
   - GET branches / commits / tree/:commit?path / blob/:hash
   - CorsLayer::permissive
2. `feat(frontend): 코드 브라우저 Web UI`
   - React(Vite) SPA, axios 클라이언트(VITE_API_URL)
   - 라우트: / (목록), /repos/:id (뷰)
   - RepoView: 브랜치 선택 → 커밋 히스토리 → 파일 브라우저(경로 네비/파일 보기)
     + 빌드 패널(HEAD 빌드 트리거, 상태 배지, 로그)

## 검증
- ✅ 서버 `cargo test` 전체 green (55).
- ✅ 읽기 엔드포인트 E2E: branches(dev/main), commits 체인, tree(root/src),
  blob(README 내용) 정상.
- ✅ 프론트 `tsc -b && vite build` 통과(타입 에러 0), preview 서빙 확인.
- ✅ 서버 CORS 헤더(`access-control-allow-origin: *`) 확인.
- (브라우저 렌더는 수동 확인 영역 — `npm run dev` 후 :5173)

## 한계 / 후속
- 실시간 갱신 없음(수동 새로고침). diff 뷰/구문 강조 미적용(plain pre).
- 인증 UI 없음(시드 유저). 저장소 생성은 CLI(`cts remote`)로.

## 로드맵 완료
Phase 1~7 모두 완료. (Core / Server CRUD / CLI 로컬 / Push·Pull / Branch /
Build / Web UI)
