import { useQuery } from '@tanstack/react-query'
import { NavLink, useNavigate } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { getCourses, getAssignments, type Assignment } from '@/lib/api'
import { useAuth } from '@/stores/auth'
import { useState } from 'react'

export function Sidebar() {
  const { logout } = useAuth()
  const navigate = useNavigate()
  const [expandedCourse, setExpandedCourse] = useState<number | null>(null)
  const [expandedAssignments, setExpandedAssignments] = useState<
    Map<number, Assignment[]>
  >(new Map())

  const { data: courses } = useQuery({
    queryKey: ['courses'],
    queryFn: getCourses,
  })

  function handleLogout() {
    logout()
    navigate('/login')
  }

  async function toggleCourse(courseId: number) {
    if (expandedCourse === courseId) {
      setExpandedCourse(null)
      return
    }
    setExpandedCourse(courseId)
    try {
      const assignments = await getAssignments(courseId)
      const next = new Map(expandedAssignments)
      next.set(courseId, assignments)
      setExpandedAssignments(next)
    } catch (e) {
      console.error('Failed to load assignments:', e)
    }
  }

  return (
    <aside className="flex w-64 flex-col border-r bg-card">
      <div className="border-b p-4">
        <h1 className="text-lg font-bold">CG Helper</h1>
        <p className="text-xs text-muted-foreground">程序设计辅助系统</p>
      </div>

      <ScrollArea className="flex-1">
        <nav className="p-2">
          {courses?.map((c) => (
            <div key={c.course_id}>
              <button
                type="button"
                className="flex w-full items-center justify-between rounded-lg px-3 py-2 text-sm hover:bg-muted"
                onClick={() => toggleCourse(c.course_id)}
              >
                <span className="truncate text-left">{c.course_name}</span>
                <span className="ml-1 text-xs text-muted-foreground">
                  {expandedCourse === c.course_id ? '▾' : '▸'}
                </span>
              </button>
              {expandedCourse === c.course_id && (
                <div className="ml-3 border-l pl-2">
                  <NavLink
                    to={`/courses/${c.course_id}/assignments`}
                    className={({ isActive }) =>
                      `block rounded px-3 py-1.5 text-sm ${
                        isActive
                          ? 'bg-primary/10 text-primary'
                          : 'text-muted-foreground hover:text-foreground'
                      }`
                    }
                  >
                    作业列表
                  </NavLink>
                  {expandedAssignments.get(c.course_id)?.map((a) => (
                    <NavLink
                      key={a.assign_id}
                      to={`/courses/${c.course_id}/assignments/${a.assign_id}/problems`}
                      className={({ isActive }) =>
                        `block rounded px-3 py-1.5 text-xs ${
                          isActive
                            ? 'bg-primary/10 text-primary'
                            : 'text-muted-foreground hover:text-foreground'
                        }`
                      }
                    >
                      {a.assign_name}
                    </NavLink>
                  ))}
                </div>
              )}
            </div>
          ))}
        </nav>
      </ScrollArea>

      <div className="border-t p-4">
        <Button variant="outline" size="sm" className="w-full" onClick={handleLogout}>
          退出登录
        </Button>
      </div>
    </aside>
  )
}
