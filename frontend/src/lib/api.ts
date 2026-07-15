const API_BASE = '/api'

interface RequestOptions {
  method?: string
  body?: unknown
  headers?: Record<string, string>
}

class ApiError extends Error {
  status: number
  constructor(message: string, status: number) {
    super(message)
    this.status = status
    this.name = 'ApiError'
  }
}

function getToken(): string | null {
  return localStorage.getItem('cg_token')
}

export function setToken(token: string) {
  localStorage.setItem('cg_token', token)
}

export function clearToken() {
  localStorage.removeItem('cg_token')
}

export function hasToken(): boolean {
  return getToken() !== null
}

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const { method = 'GET', body, headers = {} } = opts

  const token = getToken()
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

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
    throw new ApiError(err.error || res.statusText, res.status)
  }

  return res.json()
}

// Auth
export interface CaptchaResponse {
  session_id: string
  captcha_image: string
}

export interface LoginResponse {
  token: string
}

export function getCaptcha(): Promise<CaptchaResponse> {
  return request('/auth/captcha', { method: 'POST' })
}

export function login(stu_id: string, password: string, captcha_code: string, session_id: string): Promise<LoginResponse> {
  return request('/auth/login', {
    method: 'POST',
    body: { session_id, stu_id, password, captcha_code },
  })
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
  pro_num: number
  problem_id: number
  title: string
  score: number
}

export function getProblems(courseId: number, assignId: number): Promise<Problem[]> {
  return request(`/courses/${courseId}/assignments/${assignId}/problems`)
}

export interface ProblemPage {
  html: string
}

export function getProblemPage(
  courseId: number,
  assignId: number,
  proNum: number,
): Promise<ProblemPage> {
  return request(`/courses/${courseId}/assignments/${assignId}/problems/${proNum}`)
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
