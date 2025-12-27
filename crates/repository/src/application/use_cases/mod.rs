// ============================================
// Repository 유스케이스 (Use Cases)
// ============================================
// 유스케이스 = 애플리케이션이 할 수 있는 행동
// 하나의 유스케이스 = 하나의 기능
//
// 이 모듈에 정의될 유스케이스들:
//   - CreateRepositoryUseCase: 새 저장소 생성
//   - GetRepositoryUseCase: 저장소 조회
//   - ListRepositoriesUseCase: 저장소 목록
//   - DeleteRepositoryUseCase: 저장소 삭제
//   - BrowseCodeUseCase: 코드 브라우징
//
// 유스케이스 구조 예시:
//
// pub struct CreateRepositoryUseCase<R: RepositoryRepository, G: GitPort> {
//     repository_repo: R,  // DB 저장소
//     git_port: G,         // Git 작업
// }
//
// impl<R: RepositoryRepository, G: GitPort> CreateRepositoryUseCase<R, G> {
//     pub fn new(repository_repo: R, git_port: G) -> Self {
//         Self { repository_repo, git_port }
//     }
//
//     pub async fn execute(&self, input: CreateRepositoryInput) -> Result<RepositoryDto, Error> {
//         // 1. 도메인 객체 생성
//         let repo = Repository::new(input.name, input.owner_id);
//         
//         // 2. Git 저장소 초기화
//         self.git_port.init(&repo.path())?;
//         
//         // 3. DB에 저장
//         self.repository_repo.save(&repo).await?;
//         
//         // 4. DTO로 변환해서 반환
//         Ok(RepositoryDto::from(repo))
//     }
// }
