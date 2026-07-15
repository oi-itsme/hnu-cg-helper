import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'
import { setToken, clearToken, hasToken } from '@/lib/api'

interface AuthState {
  isLoggedIn: boolean
  login: (token: string) => void
  logout: () => void
}

const AuthContext = createContext<AuthState | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [isLoggedIn, setIsLoggedIn] = useState(hasToken())

  const login = useCallback((token: string) => {
    setToken(token)
    setIsLoggedIn(true)
  }, [])

  const logout = useCallback(() => {
    clearToken()
    setIsLoggedIn(false)
  }, [])

  return (
    <AuthContext.Provider value={{ isLoggedIn, login, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}
