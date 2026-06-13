// =============================================================================
// 로그인 / 회원가입 (pages/Login.tsx)
// =============================================================================

import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { login, register } from '../services'
import { useAuth } from '../stores'

export default function Login() {
  const navigate = useNavigate()
  const setAuth = useAuth((s) => s.setAuth)
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const result =
        mode === 'login'
          ? await login(username, password)
          : await register(username, email, password)
      setAuth(result.token, result.user.username)
      navigate('/')
    } catch (err: unknown) {
      const msg =
        (err as { response?: { data?: { error?: string } } })?.response?.data
          ?.error ?? '실패했습니다'
      setError(msg)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="panel" style={{ maxWidth: 380, margin: '40px auto' }}>
      <h2>{mode === 'login' ? '로그인' : '회원가입'}</h2>
      <form onSubmit={submit} style={{ padding: 16, display: 'grid', gap: 10 }}>
        <input
          placeholder="사용자명"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoFocus
        />
        {mode === 'register' && (
          <input
            placeholder="이메일"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
          />
        )}
        <input
          placeholder="비밀번호"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        {error && <div style={{ color: 'var(--red)' }}>{error}</div>}
        <button type="submit" disabled={busy}>
          {mode === 'login' ? '로그인' : '가입하기'}
        </button>
        <a
          style={{ cursor: 'pointer', fontSize: 13 }}
          onClick={() => {
            setMode(mode === 'login' ? 'register' : 'login')
            setError(null)
          }}
        >
          {mode === 'login' ? '계정이 없으신가요? 회원가입' : '이미 계정이 있으신가요? 로그인'}
        </a>
      </form>
    </div>
  )
}
