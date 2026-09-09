import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router-dom'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { getAssignments } from '@/lib/api'

export default function AssignmentsPage() {
  const navigate = useNavigate()
  const { courseId } = useParams<{ courseId: string }>()
  const cid = Number(courseId)

  const { data, isLoading, error } = useQuery({
    queryKey: ['assignments', cid],
    queryFn: () => getAssignments(cid),
  })

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">加载作业列表...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-destructive">加载失败: {error.message}</p>
      </div>
    )
  }

  return (
    <div className="container mx-auto max-w-3xl py-8">
      <button
        className="mb-4 text-sm text-muted-foreground hover:text-foreground"
        onClick={() => navigate('/courses')}
      >
        &larr; 返回课程列表
      </button>
      <h1 className="mb-6 text-2xl font-bold">作业列表</h1>
      <div className="grid gap-4">
        {data?.map((a) => (
          <Card
            key={a.id}
            className="cursor-pointer transition-shadow hover:shadow-md"
            onClick={() => navigate(`/courses/${cid}/assignments/${a.id}/problems`)}
          >
            <CardHeader>
              <CardTitle className="text-lg">{a.name}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted-foreground">作业 ID: {a.id}</p>
            </CardContent>
          </Card>
        ))}
        {data?.length === 0 && (
          <p className="text-center text-muted-foreground">暂无作业</p>
        )}
      </div>
    </div>
  )
}
