import { useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router-dom'
import { ScrollArea } from '@/components/ui/scroll-area'
import { getProblemPage } from '@/lib/api'

export default function ProblemPage() {
  const navigate = useNavigate()
  const { courseId, assignId, proNum } = useParams<{
    courseId: string
    assignId: string
    proNum: string
  }>()
  const cid = Number(courseId)
  const aid = Number(assignId)
  const pnum = Number(proNum)

  const { data, isLoading, error } = useQuery({
    queryKey: ['problem', cid, aid, pnum],
    queryFn: () => getProblemPage(cid, aid, pnum),
  })

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-muted-foreground">加载题目...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-destructive">加载失败: {error.message}</p>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="border-b px-4 py-3">
        <button
          className="text-sm text-muted-foreground hover:text-foreground"
          onClick={() =>
            navigate(`/courses/${cid}/assignments/${aid}/problems`)
          }
        >
          &larr; 返回题目列表
        </button>
      </div>
      <ScrollArea className="flex-1">
        <div
          className="p-6"
          // biome-ignore lint/security/noDangerouslySetInnerHtml: CG problem HTML is from trusted source
          dangerouslySetInnerHTML={{ __html: data?.html || '' }}
        />
      </ScrollArea>
    </div>
  )
}
