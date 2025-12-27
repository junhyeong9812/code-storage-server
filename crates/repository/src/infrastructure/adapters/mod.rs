// ============================================
// Repository 어댑터 (Adapters)
// ============================================
// 도메인 포트의 실제 구현체들
//
// 이 모듈에 정의될 어댑터들:
//   - SqliteRepositoryAdapter: SQLite로 저장소 메타데이터 저장
//   - Git2Adapter: git2-rs로 Git 작업 수행
//   - FileSystemAdapter: 파일 시스템 접근
//
// 어댑터 구현 예시:
//
// use crate::domain::ports::GitPort;
// use git2::Repository as GitRepo;
//
// pub struct Git2Adapter {
//     base_path: PathBuf,  // Git 저장소들이 저장될 경로
// }
//
// impl Git2Adapter {
//     pub fn new(base_path: PathBuf) -> Self {
//         Self { base_path }
//     }
// }
//
// impl GitPort for Git2Adapter {
//     fn init(&self, name: &str) -> Result<(), Error> {
//         let path = self.base_path.join(name);
//         GitRepo::init_bare(&path)?;  // bare repository 생성
//         Ok(())
//     }
//
//     fn get_commits(&self, name: &str, branch: &str) -> Result<Vec<Commit>, Error> {
//         let path = self.base_path.join(name);
//         let repo = GitRepo::open(&path)?;
//         // ... 커밋 조회 로직
//     }
// }
//
// 장점: Git 라이브러리를 교체해도 도메인 코드 변경 없음
//       테스트 시 Mock 어댑터로 대체 가능
