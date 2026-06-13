// =============================================================================
// CTS Web UI 라우터 (App.tsx)
// =============================================================================

import { BrowserRouter, Link, Route, Routes, useNavigate } from 'react-router-dom'
import { GitBranch, LogOut } from 'lucide-react'
import RepoList from './pages/RepoList'
import RepoView from './pages/RepoView'
import Login from './pages/Login'
import { useAuth } from './stores'
import { logout as apiLogout } from './services'

function TopBar() {
  const navigate = useNavigate()
  const { username, clear } = useAuth()

  const onLogout = async () => {
    try {
      await apiLogout()
    } catch {
      /* 무시 */
    }
    clear()
    navigate('/login')
  }

  return (
    <div className="topbar">
      <GitBranch size={20} color="#58a6ff" />
      <Link to="/" className="brand">
        Code<span>Storage</span>
      </Link>
      <span className="muted small">— 독립 버전 관리 시스템</span>
      <span className="spacer" />
      {username ? (
        <>
          <span className="small">@{username}</span>
          <button onClick={onLogout} title="로그아웃">
            <LogOut size={13} />
          </button>
        </>
      ) : (
        <Link to="/login" className="small">
          로그인
        </Link>
      )}
    </div>
  )
}

function App() {
  return (
    <BrowserRouter>
      <div className="app">
        <TopBar />
        <Routes>
          <Route path="/" element={<RepoList />} />
          <Route path="/login" element={<Login />} />
          <Route path="/repos/:id" element={<RepoView />} />
        </Routes>
      </div>
    </BrowserRouter>
  )
}

export default App
