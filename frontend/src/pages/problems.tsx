import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams, Link } from 'react-router-dom'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { getProblems } from '@/lib/api'

export default function ProblemsPage() {
  const navigate = useNavigate()
  const { courseId, assignId } = useParams<{ courseId: string; assignId: string }>()
  const cid = Number(courseId)
  const aid = Number(assignId)

  const { data, isLoading, error } = useQuery({
    queryKey: ['problems', cid, aid],
    queryFn: () => getProblems(cid, aid),
  })

  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-muted-foreground">加载题目列表...</p>
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
        onClick={() => navigate(`/courses/${cid}/assignments`)}
      >
        &larr; 返回作业列表
      </button>
      <h1 className="mb-6 text-2xl font-bold">题目列表</h1>
      <div className="grid gap-4">
        {data?.map((p) => (
          <Link
            key={p.pro_num}
            to={`/courses/${cid}/assignments/${aid}/problems/${p.pro_num}`}
          >
            <Card className="transition-shadow hover:shadow-md">
              <CardHeader>
                <CardTitle className="text-lg">
                  #{p.pro_num} {p.title}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted-foreground">
                  分值: {p.score} | 题目 ID: {p.problem_id}
                </p>
              </CardContent>
            </Card>
          </Link>
        ))}
        {data?.length === 0 && (
          <p className="text-center text-muted-foreground">暂无题目</p>
        )}
      </div>
    </div>
  )
}
