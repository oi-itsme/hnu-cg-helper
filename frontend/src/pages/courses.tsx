import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { getCourses, type Course } from '@/lib/api'
import { useAuth } from '@/stores/auth'

export default function CoursesPage() {
  const navigate = useNavigate()
  const { logout } = useAuth()
  const { data: courses, isLoading, error } = useQuery<Course[]>({
    queryKey: ['courses'],
    queryFn: getCourses,
  })

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">加载课程列表...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-4">
        <p className="text-destructive">加载失败: {error.message}</p>
        <Button variant="outline" onClick={() => { logout(); navigate('/login') }}>
          退出登录
        </Button>
      </div>
    )
  }

  return (
    <div className="container mx-auto max-w-3xl py-8">
      <h1 className="mb-6 text-2xl font-bold">我的课程</h1>
      <div className="grid gap-4">
        {courses?.map((course) => (
          <Card
            key={course.id}
            className="cursor-pointer transition-shadow hover:shadow-md"
            onClick={() => navigate(`/courses/${course.id}/assignments`)}
          >
            <CardHeader>
              <CardTitle className="text-lg">{course.name}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">课程 ID: {course.id}</p>
            </CardContent>
          </Card>
        ))}
        {courses?.length === 0 && (
          <p className="text-center text-muted-foreground">暂无课程</p>
        )}
      </div>
    </div>
  )
}
