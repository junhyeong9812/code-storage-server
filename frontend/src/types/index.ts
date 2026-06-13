// =============================================================================
// CTS API 타입 (types/index.ts)
// =============================================================================
// 서버 응답 DTO 와 1:1 대응.

export interface Repository {
  id: string
  name: string
  description: string | null
  owner_id: string
  default_branch: string
  is_private: boolean
  created_at: string
  updated_at: string
}

export interface Branch {
  name: string
  head_commit: string
}

export interface Commit {
  hash: string
  message: string
  author_name: string
  author_email: string
  timestamp: string
  parent_hash: string | null
}

export interface TreeEntry {
  name: string
  object_type: 'blob' | 'tree'
  hash: string
  mode: string
}

export interface BlobContent {
  hash: string
  size: number
  is_text: boolean
  content: string
}

export type BuildStatus = 'pending' | 'running' | 'success' | 'failed'

export interface Build {
  id: string
  repository_id: string
  commit_hash: string
  status: BuildStatus
  started_at: string | null
  finished_at: string | null
  created_at: string
}
