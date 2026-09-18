const API_BASE = '/api'

interface RequestOptions {
  method?: string
  body?: unknown
  headers?: Record<string, string>
}

class ApiError extends Error {
  status: number
  /** 是否为脚本层失败（可尝试 AI 修复） */
  repairable?: boolean
  /** 失败时的页面类型 */
  pageType?: string
  constructor(message: string, status: number, repairable?: boolean, pageType?: string) {
    super(message)
    this.status = status
    this.repairable = repairable
    this.pageType = pageType
    this.name = 'ApiError'
  }
}

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, headers = {} } = opts

  const init: RequestInit = {
    method,
    headers: {
      'Content-Type': 'application/json',
      ...headers,
    },
  }

  if (body !== undefined) {
    init.body = JSON.stringify(body)
  }

  const res = await fetch(`${API_BASE}${path}`, init)

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new ApiError(err.error || res.statusText, res.status, err.repairable, err.page_type)
  }

  return res.json()
}

// Auth
export interface CaptchaResponse {
  session_id: string
  captcha_image: string
}

export function getCaptcha(): Promise<CaptchaResponse> {
  return request('/auth/captcha', { method: 'POST' })
}

export function login(stu_id: string, password: string, captcha_code: string, session_id: string): Promise<void> {
  return request('/auth/login', {
    method: 'POST',
    body: { session_id, stu_id, password, captcha_code },
  })
}

export function logout(): Promise<void> {
  return request('/auth/logout', { method: 'POST' })
}

export interface AuthStatus {
  authenticated: boolean
}

export function getAuthStatus(): Promise<AuthStatus> {
  return request('/auth/status')
}

// Courses
export interface Course {
  id: number
  name: string
}

export function getCourses(): Promise<Course[]> {
  return request('/courses')
}

export interface Assignment {
  id: number
  name: string
}

export function getAssignments(courseId: number): Promise<Assignment[]> {
  return request(`/courses/${courseId}/assignments`)
}

export interface Problem {
  index: number
  id: number
  title: string
  score: number
}

export function getProblems(courseId: number, assignId: number): Promise<Problem[]> {
  return request(`/courses/${courseId}/assignments/${assignId}/problems`)
}

// ── 题目页（适配框架结构化输出） ─────────────────────────

export interface Language {
  value: string
  label: string
}

export interface Gap {
  name: string
}

export type SkeletonPart =
  | { type: 'code'; text: string }
  | { type: 'gap'; name: string }

export type SubmissionDescriptor =
  | {
      kind: 'file_upload'
      languages: Language[]
      needs_main_class: boolean
      problem_id: number
      assign_id: number
    }
  | {
      kind: 'fill_gap'
      languages: Language[]
      gaps: Gap[]
      skeleton: SkeletonPart[]
      hidden_fields: Record<string, string>
      problem_id: number
      assign_id: number
    }

export interface ProblemView {
  page_type: string
  statement_html: string
  statement_text: string
  submission: SubmissionDescriptor
}

export function getProblemPage(
  courseId: number,
  assignId: number,
  proNum: number,
): Promise<ProblemView> {
  return request(`/courses/${courseId}/assignments/${assignId}/problems/${proNum}`)
}

export interface SubmitPayload {
  page_type: string
  descriptor: SubmissionDescriptor
  language: string
  main_class?: string
  code?: string
  answers?: Record<string, string>
  wtime: number
}

export interface SubmitResponse {
  result_html: string
}

export function submitProblem(
  courseId: number,
  assignId: number,
  proNum: number,
  payload: SubmitPayload,
): Promise<SubmitResponse> {
  return request(
    `/courses/${courseId}/assignments/${assignId}/problems/${proNum}/submit`,
    { method: 'POST', body: payload },
  )
}

// ── AI 修复 ─────────────────────────────────────────────

export interface RepairEvent {
  stage: 'attempt' | 'validating' | 'done' | 'failed'
  attempt: number
  message: string
}

/** SSE 流式接收修复进度 */
export async function* repairAdapter(pageType: string): AsyncGenerator<RepairEvent> {
  const res = await fetch(`${API_BASE}/adapter/repair`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ page_type: pageType }),
  })

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new ApiError(err.error || res.statusText, res.status)
  }

  const reader = res.body?.getReader()
  if (!reader) throw new Error('No response body')

  const decoder = new TextDecoder()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        try {
          yield JSON.parse(line.slice(6))
        } catch {
          // skip parse errors
        }
      }
    }
  }
}

// AI
export interface ChatMessage {
  role: string
  content: string
}

export async function* streamChat(
  messages: ChatMessage[],
): AsyncGenerator<{ content: string; finish_reason?: string }> {
  const res = await fetch(`${API_BASE}/ai/chat`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ messages }),
  })

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new ApiError(err.error || res.statusText, res.status)
  }

  const reader = res.body?.getReader()
  if (!reader) throw new Error('No response body')

  const decoder = new TextDecoder()
  let buffer = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })
    const lines = buffer.split('\n')
    buffer = lines.pop() || ''

    for (const line of lines) {
      if (line.startsWith('data: ')) {
        try {
          const data = JSON.parse(line.slice(6))
          yield data
        } catch {
          // skip parse errors
        }
      }
    }
  }
}

export interface AiConfig {
  has_api_key: boolean
  base_url: string
  model: string
}

export function getAiConfig(): Promise<AiConfig> {
  return request('/ai/config')
}

export function setAiConfig(opts: {
  api_key?: string
  base_url?: string
  model?: string
}): Promise<AiConfig> {
  return request('/ai/config', { method: 'POST', body: opts })
}
