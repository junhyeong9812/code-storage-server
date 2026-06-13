// =============================================================================
// 저장소 뷰 — 코드 브라우저 (pages/RepoView.tsx)
// =============================================================================
//
// 브랜치 선택 → 커밋 히스토리 → 트리/파일 브라우징 + 빌드 패널.

import { useCallback, useEffect, useState } from 'react'
import { useParams, Link } from 'react-router-dom'
import {
  File as FileIcon,
  Folder,
  GitCommit,
  Hammer,
  RefreshCw,
} from 'lucide-react'
import dayjs from 'dayjs'
import {
  getBlob,
  getBranches,
  getBuildLog,
  getBuilds,
  getCommits,
  getRepository,
  getTree,
  triggerBuild,
} from '../services'
import type {
  BlobContent,
  Branch,
  Build,
  Commit,
  Repository,
  TreeEntry,
} from '../types'

const short = (h: string) => h.slice(0, 8)

export default function RepoView() {
  const { id = '' } = useParams()
  const [repo, setRepo] = useState<Repository | null>(null)
  const [branches, setBranches] = useState<Branch[]>([])
  const [branch, setBranch] = useState<string>('')
  const [commits, setCommits] = useState<Commit[]>([])
  const [selectedCommit, setSelectedCommit] = useState<string>('')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getRepository(id)
      .then((r) => {
        setRepo(r)
        setBranch(r.default_branch)
      })
      .catch((e) => setError(e?.message ?? '저장소 로드 실패'))
    getBranches(id).then(setBranches).catch(() => {})
  }, [id])

  useEffect(() => {
    if (!branch) return
    getCommits(id, branch)
      .then((cs) => {
        setCommits(cs)
        setSelectedCommit(cs[0]?.hash ?? '')
      })
      .catch(() => {
        setCommits([])
        setSelectedCommit('')
      })
  }, [id, branch])

  if (error) return <div className="empty">{error}</div>
  if (!repo) return <div className="empty">불러오는 중…</div>

  return (
    <div>
      <div style={{ marginBottom: 16, display: 'flex', alignItems: 'center', gap: 12 }}>
        <h1 style={{ fontSize: 22, margin: 0 }}>
          <Link to="/">저장소</Link> / {repo.name}
        </h1>
        <select value={branch} onChange={(e) => setBranch(e.target.value)}>
          {branches.map((b) => (
            <option key={b.name} value={b.name}>
              {b.name}
            </option>
          ))}
        </select>
        <span className="muted small">
          {branches.length} branches · {commits.length} commits
        </span>
      </div>

      <div className="grid">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
          {selectedCommit && (
            <FileBrowser key={selectedCommit} id={id} commit={selectedCommit} />
          )}
          <CommitList
            commits={commits}
            selected={selectedCommit}
            onSelect={setSelectedCommit}
          />
        </div>
        <BuildsPanel id={id} headCommit={commits[0]?.hash ?? ''} />
      </div>
    </div>
  )
}

// -----------------------------------------------------------------------------
// 파일 브라우저
// -----------------------------------------------------------------------------
function FileBrowser({ id, commit }: { id: string; commit: string }) {
  const [path, setPath] = useState('')
  const [entries, setEntries] = useState<TreeEntry[]>([])
  const [blob, setBlob] = useState<{ name: string; data: BlobContent } | null>(null)

  useEffect(() => {
    setBlob(null)
    getTree(id, commit, path).then(setEntries).catch(() => setEntries([]))
  }, [id, commit, path])

  const openEntry = (e: TreeEntry) => {
    if (e.object_type === 'tree') {
      setPath(path ? `${path}/${e.name}` : e.name)
    } else {
      getBlob(id, e.hash).then((data) => setBlob({ name: e.name, data }))
    }
  }

  const segments = path ? path.split('/') : []
  const gotoDepth = (depth: number) => {
    setBlob(null)
    setPath(segments.slice(0, depth).join('/'))
  }

  return (
    <div className="panel">
      <div className="breadcrumb">
        <span className="mono">@ {short(commit)}</span>{'  '}
        <a onClick={() => gotoDepth(0)} style={{ cursor: 'pointer' }}>
          root
        </a>
        {segments.map((s, i) => (
          <span key={i}>
            {' / '}
            <a onClick={() => gotoDepth(i + 1)} style={{ cursor: 'pointer' }}>
              {s}
            </a>
          </span>
        ))}
      </div>

      {blob ? (
        <div>
          <div className="row">
            <FileIcon size={15} color="#8b949e" />
            <strong>{blob.name}</strong>
            <span className="muted small">{blob.data.size} bytes</span>
            <span className="spacer" />
            <button onClick={() => setBlob(null)}>닫기</button>
          </div>
          {blob.data.is_text ? (
            <pre className="code">{blob.data.content}</pre>
          ) : (
            <div className="empty">바이너리 파일입니다.</div>
          )}
        </div>
      ) : (
        <div>
          {entries.length === 0 && <div className="empty">(빈 디렉토리)</div>}
          {entries.map((e) => (
            <div className="row entry" key={e.name} onClick={() => openEntry(e)}>
              {e.object_type === 'tree' ? (
                <Folder size={15} color="#58a6ff" />
              ) : (
                <FileIcon size={15} color="#8b949e" />
              )}
              <span className="name">{e.name}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// -----------------------------------------------------------------------------
// 커밋 목록
// -----------------------------------------------------------------------------
function CommitList({
  commits,
  selected,
  onSelect,
}: {
  commits: Commit[]
  selected: string
  onSelect: (hash: string) => void
}) {
  return (
    <div className="panel">
      <h3>커밋 히스토리</h3>
      {commits.length === 0 && <div className="empty">커밋이 없습니다.</div>}
      {commits.map((c) => (
        <div
          className="row entry"
          key={c.hash}
          onClick={() => onSelect(c.hash)}
          style={{ background: c.hash === selected ? '#1c2330' : undefined }}
        >
          <GitCommit size={15} color="#8b949e" />
          <div style={{ minWidth: 0 }}>
            <div className="commit-msg">{c.message.split('\n')[0]}</div>
            <div className="muted small">
              <span className="mono">{short(c.hash)}</span> · {c.author_name} ·{' '}
              {dayjs(c.timestamp).format('YYYY-MM-DD HH:mm')}
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}

// -----------------------------------------------------------------------------
// 빌드 패널
// -----------------------------------------------------------------------------
function BuildsPanel({ id, headCommit }: { id: string; headCommit: string }) {
  const [builds, setBuilds] = useState<Build[]>([])
  const [log, setLog] = useState<{ id: string; text: string } | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(() => {
    getBuilds(id).then(setBuilds).catch(() => setBuilds([]))
  }, [id])

  useEffect(() => {
    refresh()
  }, [refresh])

  const onTrigger = async () => {
    if (!headCommit) return
    setBusy(true)
    try {
      await triggerBuild(id, headCommit)
      refresh()
    } catch {
      /* 무시 */
    } finally {
      setBusy(false)
    }
  }

  const openLog = (buildId: string) => {
    getBuildLog(id, buildId).then((text) => setLog({ id: buildId, text }))
  }

  return (
    <div className="panel">
      <h3>
        빌드{' '}
        <button
          onClick={onTrigger}
          disabled={busy || !headCommit}
          style={{ marginLeft: 8 }}
        >
          <Hammer size={12} /> HEAD 빌드
        </button>
        <button onClick={refresh} style={{ marginLeft: 4 }}>
          <RefreshCw size={12} />
        </button>
      </h3>
      {builds.length === 0 && <div className="empty">빌드 없음</div>}
      {builds.map((b) => (
        <div className="row entry" key={b.id} onClick={() => openLog(b.id)}>
          <span className={`badge ${b.status}`}>{b.status}</span>
          <span className="mono small">{short(b.commit_hash)}</span>
          <span className="spacer" />
          <span className="muted small">
            {dayjs(b.created_at).format('HH:mm:ss')}
          </span>
        </div>
      ))}
      {log && (
        <div>
          <div className="row">
            <strong className="small">빌드 로그 {short(log.id)}</strong>
            <span className="spacer" />
            <button onClick={() => setLog(null)}>닫기</button>
          </div>
          <pre className="code" style={{ maxHeight: 300 }}>
            {log.text || '(로그 없음)'}
          </pre>
        </div>
      )}
    </div>
  )
}
