-- =============================================================================
-- CTS Database Schema
-- =============================================================================

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ---------------------------------------------------------------------------
-- users (사용자)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- repositories (저장소)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS repositories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    default_branch VARCHAR(100) NOT NULL DEFAULT 'main',
    is_private BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_repositories_owner_name UNIQUE (owner_id, name)
);

-- ---------------------------------------------------------------------------
-- blobs (파일 내용)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    hash VARCHAR(64) NOT NULL,      -- SHA-256 해시
    size BIGINT NOT NULL,
    storage_path VARCHAR(500) NOT NULL,  -- 실제 파일 저장 경로
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_blobs_repo_hash UNIQUE (repository_id, hash)
);

-- ---------------------------------------------------------------------------
-- trees (디렉토리 구조)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS trees (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    hash VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_trees_repo_hash UNIQUE (repository_id, hash)
);

-- ---------------------------------------------------------------------------
-- tree_entries (트리 항목 - 파일/폴더)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tree_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tree_id UUID NOT NULL REFERENCES trees(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    mode VARCHAR(10) NOT NULL,       -- 'blob', 'tree'
    target_type VARCHAR(10) NOT NULL, -- 'blob', 'tree'
    target_id UUID NOT NULL,          -- blob_id 또는 tree_id
    CONSTRAINT uk_tree_entries UNIQUE (tree_id, name)
);

-- ---------------------------------------------------------------------------
-- commits (커밋)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS commits (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    hash VARCHAR(64) NOT NULL,
    tree_id UUID NOT NULL REFERENCES trees(id),
    parent_id UUID REFERENCES commits(id),  -- 첫 커밋은 NULL
    message TEXT NOT NULL,
    author_name VARCHAR(100) NOT NULL,
    author_email VARCHAR(255) NOT NULL,
    committed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_commits_repo_hash UNIQUE (repository_id, hash)
);

-- ---------------------------------------------------------------------------
-- branches (브랜치)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS branches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    head_commit_id UUID REFERENCES commits(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_branches_repo_name UNIQUE (repository_id, name)
);

-- ---------------------------------------------------------------------------
-- tags (태그)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    commit_id UUID NOT NULL REFERENCES commits(id),
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uk_tags_repo_name UNIQUE (repository_id, name)
);

-- ---------------------------------------------------------------------------
-- builds (CI/CD 빌드)
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS builds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository_id UUID NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    commit_id UUID NOT NULL REFERENCES commits(id),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    log_path VARCHAR(500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Indexes
-- ---------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_repositories_owner ON repositories(owner_id);
CREATE INDEX IF NOT EXISTS idx_blobs_repository ON blobs(repository_id);
CREATE INDEX IF NOT EXISTS idx_trees_repository ON trees(repository_id);
CREATE INDEX IF NOT EXISTS idx_commits_repository ON commits(repository_id);
CREATE INDEX IF NOT EXISTS idx_commits_parent ON commits(parent_id);
CREATE INDEX IF NOT EXISTS idx_branches_repository ON branches(repository_id);
CREATE INDEX IF NOT EXISTS idx_builds_repository ON builds(repository_id);
CREATE INDEX IF NOT EXISTS idx_builds_status ON builds(status);

-- ---------------------------------------------------------------------------
-- Triggers (updated_at 자동 갱신)
-- ---------------------------------------------------------------------------
CREATE OR REPLACE FUNCTION update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER tr_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER tr_repositories_updated_at BEFORE UPDATE ON repositories
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER tr_branches_updated_at BEFORE UPDATE ON branches
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

CREATE TRIGGER tr_builds_updated_at BEFORE UPDATE ON builds
    FOR EACH ROW EXECUTE FUNCTION update_updated_at();

-- ---------------------------------------------------------------------------
-- Test Data
-- ---------------------------------------------------------------------------
INSERT INTO users (id, username, email, password_hash)
VALUES ('00000000-0000-0000-0000-000000000001', 'testuser', 'test@example.com', 'dummy')
ON CONFLICT DO NOTHING;
