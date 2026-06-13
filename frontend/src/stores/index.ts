// =============================================================================
// 인증 스토어 (stores/index.ts)
// =============================================================================
// zustand 로 토큰/사용자명을 관리하고 localStorage 에 영속한다.

import { create } from 'zustand'

const TOKEN_KEY = 'cts_token'
const USER_KEY = 'cts_user'

interface AuthState {
  token: string | null
  username: string | null
  setAuth: (token: string, username: string) => void
  clear: () => void
}

export const useAuth = create<AuthState>((set) => ({
  token: localStorage.getItem(TOKEN_KEY),
  username: localStorage.getItem(USER_KEY),
  setAuth: (token, username) => {
    localStorage.setItem(TOKEN_KEY, token)
    localStorage.setItem(USER_KEY, username)
    set({ token, username })
  },
  clear: () => {
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(USER_KEY)
    set({ token: null, username: null })
  },
}))
