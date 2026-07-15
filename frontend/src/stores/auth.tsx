import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react'
import { getAuthStatus, logout as apiLogout } from '@/lib/api'

interface AuthState {
  isLoggedIn: boolean
  loading: boolean
  login: () => void
  logout: () => void
}

const AuthContext = createContext<AuthState | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [isLoggedIn, setIsLoggedIn] = useState(false)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    getAuthStatus()
      .then((res) => setIsLoggedIn(res.authenticated))
      .catch(() => setIsLoggedIn(false))
      .finally(() => setLoading(false))
  }, [])

  const login = useCallback(() => {
    setIsLoggedIn(true)
  }, [])

  const logout = useCallback(() => {
    apiLogout().catch(() => {})
    setIsLoggedIn(false)
  }, [])

  return (
    <AuthContext.Provider value={{ isLoggedIn, loading, login, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
