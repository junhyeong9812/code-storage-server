// =============================================================================
// CTS Web UI 라우터 (App.tsx)
// =============================================================================

import { BrowserRouter, Link, Route, Routes } from 'react-router-dom'
import { GitBranch } from 'lucide-react'
import RepoList from './pages/RepoList'
import RepoView from './pages/RepoView'

function App() {
  return (
    <BrowserRouter>
      <div className="app">
        <div className="topbar">
          <GitBranch size={20} color="#58a6ff" />
          <Link to="/" className="brand">
            Code<span>Storage</span>
          </Link>
          <span className="muted small">— 독립 버전 관리 시스템</span>
        </div>
        <Routes>
          <Route path="/" element={<RepoList />} />
          <Route path="/repos/:id" element={<RepoView />} />
        </Routes>
      </div>
    </BrowserRouter>
  )
}

export default App
