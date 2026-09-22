import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { useNavigate, useParams } from 'react-router-dom'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import {
  getProblemPage,
  repairAdapter,
  submitProblem,
  type ProblemView,
  type RepairEvent,
} from '@/lib/api'
import { useProblemContext } from '@/stores/problem-context'

/** 题目详情页：渲染适配框架产出的结构化题面 + 提交区 */
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

  const { data, isLoading, error, refetch } = useQuery({
    queryKey: ['problem', cid, aid, pnum],
    queryFn: () => getProblemPage(cid, aid, pnum),
    retry: false,
  })

  const { setProblem, clearProblem } = useProblemContext()
  useEffect(() => {
    if (data) {
      setProblem(data.statement_text, `题目 #${pnum}`)
      return () => clearProblem()
    }
  }, [data, pnum, setProblem, clearProblem])

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-muted-foreground">加载题目...</p>
      </div>
    )
  }

  if (error || !data) {
    return (
      <FailureView
        error={error}
        onRepaired={() => refetch()}
        onBack={() => navigate(`/courses/${cid}/assignments/${aid}/problems`)}
      />
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b px-4 py-3">
        <button
          className="text-sm text-muted-foreground hover:text-foreground"
          onClick={() => navigate(`/courses/${cid}/assignments/${aid}/problems`)}
        >
          &larr; 返回题目列表
        </button>
        <span className="text-xs text-muted-foreground">题型: {data.page_type}</span>
      </div>
      <ScrollArea className="flex-1">
        <div className="mx-auto max-w-4xl p-6">
          {/* 题面：语义化 HTML，视觉风格由 cg-prose 决定 */}
          <div
            className="cg-prose"
            // biome-ignore lint/security/noDangerouslySetInnerHtml: 已经过服务端白名单清洗
            dangerouslySetInnerHTML={{ __html: data.statement_html }}
          />
          <SubmissionPanel key={pnum} view={data} cid={cid} aid={aid} pnum={pnum} />
        </div>
      </ScrollArea>
    </div>
  )
}

/** 解析失败视图：按错误类型分派（登录过期 / 适配器故障 / 其他可重试错误） */
function FailureView({
  error,
  onRepaired,
  onBack,
}: {
  error: unknown
  onRepaired: () => void
  onBack: () => void
}) {
  const apiErr = error as {
    message?: string
    status?: number
    repairable?: boolean
    pageType?: string
  }
  const navigate = useNavigate()
  const [repairing, setRepairing] = useState(false)
  const [events, setEvents] = useState<RepairEvent[]>([])
  const done = events.some((e) => e.stage === 'done')
  const failed = events.some((e) => e.stage === 'failed')

  async function startRepair() {
    if (!apiErr?.pageType) return
    setRepairing(true)
    setEvents([])
    try {
      for await (const ev of repairAdapter(apiErr.pageType)) {
        setEvents((prev) => [...prev, ev])
      }
    } catch (e) {
      setEvents((prev) => [
        ...prev,
        {
          stage: 'failed',
          attempt: 0,
          message: e instanceof Error ? e.message : '修复请求失败',
        },
      ])
    } finally {
      setRepairing(false)
    }
  }

  // 登录过期：引导重新登录，而非误报为适配器故障
  if (apiErr?.status === 401) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-8">
        <div className="max-w-lg text-center">
          <h2 className="mb-2 text-lg font-semibold">登录状态已过期</h2>
          <p className="text-sm text-muted-foreground">请重新登录后再访问题目。</p>
        </div>
        <Button onClick={() => navigate('/login')}>重新登录</Button>
        <button className="text-sm text-muted-foreground hover:text-foreground" onClick={onBack}>
          &larr; 返回题目列表
        </button>
      </div>
    )
  }

  // 非适配器错误（网络/服务器故障等）：提供重试
  if (apiErr?.status !== 422) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-8">
        <div className="max-w-lg text-center">
          <h2 className="mb-2 text-lg font-semibold text-destructive">题目加载失败</h2>
          <p className="text-sm text-muted-foreground">{apiErr?.message ?? '未知错误'}</p>
        </div>
        <Button onClick={onRepaired}>重试</Button>
        <button className="text-sm text-muted-foreground hover:text-foreground" onClick={onBack}>
          &larr; 返回题目列表
        </button>
      </div>
    )
  }

  // 422：适配管线故障（脚本层可 AI 修复，固定层不可）
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 p-8">
      <div className="max-w-lg text-center">
        <h2 className="mb-2 text-lg font-semibold text-destructive">题目解析失败</h2>
        <p className="mb-1 text-sm text-muted-foreground">{apiErr?.message ?? '未知错误'}</p>
        {apiErr?.repairable && !done && (
          <p className="text-xs text-muted-foreground">
            这是脚本层故障，可以让内置 AI 尝试自动修复。
          </p>
        )}
        {apiErr?.repairable === false && (
          <p className="text-xs text-muted-foreground">
            这是固定层故障，超出自动修复能力，请反馈开发者。
          </p>
        )}
      </div>

      {apiErr?.repairable && !done && (
        <Button onClick={startRepair} disabled={repairing}>
          {repairing ? 'AI 修复中...' : '让 AI 修复'}
        </Button>
      )}
      {done && <Button onClick={onRepaired}>修复完成，重新加载</Button>}
      {failed && <p className="text-sm text-destructive">自动修复失败，请反馈开发者</p>}

      {events.length > 0 && (
        <div className="w-full max-w-lg rounded border bg-muted/50 p-3 font-mono text-xs">
          {events.map((e, i) => (
            <p key={i} className={e.stage === 'failed' ? 'text-destructive' : ''}>
              [{e.stage}{e.attempt > 0 ? ` ${e.attempt}` : ''}] {e.message}
            </p>
          ))}
        </div>
      )}

      <button className="text-sm text-muted-foreground hover:text-foreground" onClick={onBack}>
        &larr; 返回题目列表
      </button>
    </div>
  )
}

/** 提交区：按 submission.kind 分派 */
function SubmissionPanel({
  view,
  cid,
  aid,
  pnum,
}: {
  view: ProblemView
  cid: number
  aid: number
  pnum: number
}) {
  const startRef = useRef(Date.now())
  const [language, setLanguage] = useState(view.submission.languages[0]?.value ?? '')
  const [mainClass, setMainClass] = useState('')
  const [code, setCode] = useState('')
  const [answers, setAnswers] = useState<Record<string, string>>({})

  const submit = useMutation({
    mutationFn: () => {
      const wtime = Math.floor((Date.now() - startRef.current) / 1000)
      const sub = view.submission
      return submitProblem(cid, aid, pnum, {
        page_type: view.page_type,
        descriptor: sub,
        language,
        main_class: mainClass || undefined,
        code: sub.kind === 'file_upload' ? code : undefined,
        answers: sub.kind === 'fill_gap' ? answers : undefined,
        wtime,
      })
    },
  })

  const sub = view.submission
  const showMainClass =
    sub.kind === 'file_upload' && sub.needs_main_class && language === 'java'

  return (
    <div className="mt-8 rounded-lg border">
      <div className="border-b bg-muted/50 px-4 py-2 font-semibold">提交作答</div>
      <div className="space-y-4 p-4">
        {/* 语言选择（两种题型都有，填空题通常只有一种） */}
        {sub.languages.length > 1 ? (
          <label className="flex items-center gap-2 text-sm">
            编程语言
            <select
              className="rounded border bg-background px-2 py-1"
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
            >
              {sub.languages.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className="text-sm text-muted-foreground">
            编程语言: {sub.languages[0]?.label}
          </p>
        )}

        {showMainClass && (
          <input
            className="w-full rounded border px-3 py-1.5 text-sm"
            placeholder="Java 主类名（带包名格式: 包名.主类名）"
            value={mainClass}
            onChange={(e) => setMainClass(e.target.value)}
          />
        )}

        {sub.kind === 'file_upload' && (
          <textarea
            className="h-64 w-full rounded border bg-background p-3 font-mono text-sm"
            placeholder="粘贴源代码..."
            value={code}
            onChange={(e) => setCode(e.target.value)}
          />
        )}

        {sub.kind === 'fill_gap' && (
          <div className="rounded bg-muted/30 p-3 font-mono text-sm">
            {sub.skeleton.map((part, i) =>
              part.type === 'code' ? (
                <pre key={i} className="whitespace-pre-wrap">
                  {part.text}
                </pre>
              ) : (
                <textarea
                  key={i}
                  className="my-1 block w-full rounded border-2 border-primary/40 bg-background p-2"
                  placeholder={`填写 ${part.name}`}
                  rows={3}
                  value={answers[part.name] ?? ''}
                  onChange={(e) =>
                    setAnswers((prev) => ({ ...prev, [part.name]: e.target.value }))
                  }
                />
              ),
            )}
          </div>
        )}

        <Button
          onClick={() => submit.mutate()}
          disabled={submit.isPending || (sub.kind === 'file_upload' && !code.trim())}
        >
          {submit.isPending ? '提交中...' : '提交'}
        </Button>

        {submit.isError && (
          <p className="text-sm text-destructive">
            提交失败: {submit.error instanceof Error ? submit.error.message : '未知错误'}
          </p>
        )}

        {submit.isSuccess && (
          <div className="space-y-2">
            <p className="text-sm font-semibold text-green-600">已提交，判题结果：</p>
            <iframe
              title="判题结果"
              sandbox=""
              srcDoc={submit.data.result_html}
              className="h-72 w-full rounded border bg-white"
            />
          </div>
        )}
      </div>
    </div>
  )
}
