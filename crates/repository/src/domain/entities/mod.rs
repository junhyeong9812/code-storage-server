// ============================================
// Repository 엔티티
// ============================================
// 엔티티(Entity): 고유 식별자(ID)를 가진 객체
// 같은 ID면 같은 객체 (속성이 달라도)
//
// 이 모듈에 정의될 엔티티들:
//   - Repository: Git 저장소
//   - Commit: 커밋
//   - Branch: 브랜치
//
// 예시 (나중에 구현):
//
// pub struct Repository {
//     pub id: RepositoryId,       // 고유 식별자
//     pub name: String,           // 저장소 이름
//     pub description: Option<String>,
//     pub owner_id: UserId,       // 소유자
//     pub created_at: Timestamp,
//     pub updated_at: Timestamp,
// }

mod repository;