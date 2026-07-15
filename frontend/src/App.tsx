import { Routes, Route, Navigate } from 'react-router-dom'
import { AuthProvider, useAuth } from '@/stores/auth'
import type { ReactNode } from 'react'
import LoginPage from '@/pages/login'
import CoursesPage from '@/pages/courses'
import AssignmentsPage from '@/pages/assignments'
import ProblemsPage from '@/pages/problems'
import ProblemPage from '@/pages/problem'
import { AIPanel } from '@/components/ai-panel'
import { Sidebar } from '@/components/sidebar'

function LoadingScreen() {
  return (
    <div className="flex min-h-screen items-center justify-center">
      <p className="text-muted-foreground">正在检查登录状态...</p>
    </div>
  )
}

function ProtectedRoute({ children }: { children: ReactNode }) {
  const { isLoggedIn, loading } = useAuth()
  if (loading) return <LoadingScreen />
  if (!isLoggedIn) return <Navigate to="/login" replace />
  return <>{children}</>
}

function AppLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-screen">
      <Sidebar />
      <main className="flex-1 overflow-hidden">{children}</main>
      <AIPanel />
    </div>
  )
}

function AppRoutes() {
  const { isLoggedIn, loading } = useAuth()

  if (loading) return <LoadingScreen />

  return (
    <Routes>
      <Route
        path="/login"
        element={isLoggedIn ? <Navigate to="/courses" replace /> : <LoginPage />}
      />
      <Route
        path="/courses"
        element={
          <ProtectedRoute>
            <AppLayout><CoursesPage /></AppLayout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/courses/:courseId/assignments"
        element={
          <ProtectedRoute>
            <AppLayout><AssignmentsPage /></AppLayout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/courses/:courseId/assignments/:assignId/problems"
        element={
          <ProtectedRoute>
            <AppLayout><ProblemsPage /></AppLayout>
          </ProtectedRoute>
        }
      />
      <Route
        path="/courses/:courseId/assignments/:assignId/problems/:proNum"
        element={
          <ProtectedRoute>
            <AppLayout><ProblemPage /></AppLayout>
          </ProtectedRoute>
        }
      />
      <Route path="*" element={<Navigate to="/courses" replace />} />
    </Routes>
  )
}

export default function App() {
  return (
    <AuthProvider>
      <AppRoutes />
    </AuthProvider>
  )
}
