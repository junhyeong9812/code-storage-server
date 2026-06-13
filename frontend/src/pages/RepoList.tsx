// =============================================================================
// 저장소 목록 (pages/RepoList.tsx)
// =============================================================================

import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { Lock, Globe, FolderGit2 } from 'lucide-react'
import dayjs from 'dayjs'
import { getRepositories } from '../services'
import type { Repository } from '../types'

export default function RepoList() {
  const [repos, setRepos] = useState<Repository[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getRepositories()
      .then(setRepos)
      .catch((e) => setError(e?.message ?? '불러오기 실패'))
  }, [])

  if (error) return <div className="empty">서버 연결 실패: {error}</div>
  if (!repos) return <div className="empty">불러오는 중…</div>

  return (
    <div className="panel">
      <h2>저장소 ({repos.length})</h2>
      {repos.length === 0 && (
        <div className="empty">
          저장소가 없습니다. CLI 에서 <code>cts remote &lt;url&gt; &lt;name&gt;</code> 로 만드세요.
        </div>
      )}
      {repos.map((r) => (
        <div className="row" key={r.id}>
          <FolderGit2 size={16} color="#8b949e" />
          <Link to={`/repos/${r.id}`}>{r.name}</Link>
          {r.is_private ? (
            <Lock size={13} color="#8b949e" />
          ) : (
            <Globe size={13} color="#8b949e" />
          )}
          <span className="muted small">{r.description ?? ''}</span>
          <span className="spacer" />
          <span className="muted small">
            {dayjs(r.updated_at).format('YYYY-MM-DD HH:mm')}
          </span>
        </div>
      ))}
    </div>
  )
}
