import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { streamChat, getAiConfig, setAiConfig, type ChatMessage } from '@/lib/api'

const SYSTEM_PROMPT: ChatMessage = {
  role: 'system',
  content:
    '你是一个程序设计课程的助教，帮助学生理解题目、分析算法和调试代码。请用中文回答。',
}

export function AIPanel() {
  const queryClient = useQueryClient()
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([])
  const [chatInput, setChatInput] = useState('')
  const [chatStreaming, setChatStreaming] = useState(false)
  const [collapsed, setCollapsed] = useState(true)
  const [showSettings, setShowSettings] = useState(false)

  // Settings form state
  const [apiKeyInput, setApiKeyInput] = useState('')
  const [baseUrlInput, setBaseUrlInput] = useState('')
  const [modelInput, setModelInput] = useState('')

  // Fetch server-side AI config
  const { data: aiConfig, isLoading: configLoading } = useQuery({
    queryKey: ['ai-config'],
    queryFn: getAiConfig,
  })

  const saveConfig = useMutation({
    mutationFn: setAiConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ai-config'] })
      setShowSettings(false)
      setApiKeyInput('')
    },
  })

  function openSettings() {
    if (aiConfig) {
      setBaseUrlInput(aiConfig.base_url)
      setModelInput(aiConfig.model)
    }
    setShowSettings(true)
  }

  function handleSaveConfig() {
    saveConfig.mutate({
      api_key: apiKeyInput || undefined,
      base_url: baseUrlInput || undefined,
      model: modelInput || undefined,
    })
  }

  const hasApiKey = aiConfig?.has_api_key ?? false

  async function handleChatSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!chatInput.trim() || !hasApiKey || chatStreaming) return

    const userMsg: ChatMessage = { role: 'user', content: chatInput }
    const newMessages = [...chatMessages, userMsg]
    setChatMessages(newMessages)
    setChatInput('')
    setChatStreaming(true)

    const fullMessages = [SYSTEM_PROMPT, ...newMessages]
    const assistantMsg: ChatMessage = { role: 'assistant', content: '' }
    setChatMessages([...newMessages, assistantMsg])

    try {
      const stream = streamChat(fullMessages)
      let fullContent = ''
      for await (const chunk of stream) {
        fullContent += chunk.content
        setChatMessages([...newMessages, { role: 'assistant', content: fullContent }])
        if (chunk.finish_reason) break
      }
    } catch (err) {
      setChatMessages([
        ...newMessages,
        {
          role: 'assistant',
          content: `[错误] ${err instanceof Error ? err.message : '请求失败'}`,
        },
      ])
    } finally {
      setChatStreaming(false)
    }
  }

  return (
    <>
      {/* Toggle button when collapsed */}
      {collapsed && (
        <div className="flex w-12 flex-col items-center border-l bg-card pt-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setCollapsed(false)}
            title="AI 助手"
          >
            AI
          </Button>
        </div>
      )}

      {/* Expanded panel */}
      {!collapsed && (
        <div className="flex w-96 flex-col border-l bg-card">
          <div className="flex items-center justify-between border-b p-4">
            <h2 className="font-semibold">AI 助手</h2>
            <div className="flex gap-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={openSettings}
              >
                设置
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setCollapsed(true)}
              >
                ✕
              </Button>
            </div>
          </div>

          {showSettings && (
            <div className="border-b p-4 space-y-2">
              <div>
                <label className="text-xs text-muted-foreground">API Key</label>
                <input
                  type="password"
                  placeholder={hasApiKey ? '已配置（留空不修改）' : '请输入 API Key'}
                  value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  className="w-full rounded border px-3 py-1.5 text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-muted-foreground">Base URL</label>
                <input
                  type="text"
                  placeholder="https://api.deepseek.com"
                  value={baseUrlInput}
                  onChange={(e) => setBaseUrlInput(e.target.value)}
                  className="w-full rounded border px-3 py-1.5 text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-muted-foreground">Model</label>
                <input
                  type="text"
                  placeholder="deepseek-chat"
                  value={modelInput}
                  onChange={(e) => setModelInput(e.target.value)}
                  className="w-full rounded border px-3 py-1.5 text-sm"
                />
              </div>
              <div className="flex gap-2">
                <Button
                  size="sm"
                  onClick={handleSaveConfig}
                  disabled={saveConfig.isPending}
                  className="flex-1"
                >
                  {saveConfig.isPending ? '保存中...' : '保存'}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setShowSettings(false)}
                >
                  取消
                </Button>
              </div>
            </div>
          )}

          <ScrollArea className="flex-1 p-4">
            {configLoading ? (
              <p className="text-center text-sm text-muted-foreground">加载配置...</p>
            ) : !hasApiKey ? (
              <p className="text-center text-sm text-muted-foreground">
                请先
                <button
                  className="mx-1 underline hover:text-foreground"
                  onClick={openSettings}
                >
                  设置 API Key
                </button>
                以使用 AI 助手
              </p>
            ) : chatMessages.length === 0 ? (
              <p className="text-center text-sm text-muted-foreground">
                AI 助手可帮助你分析题目、理解算法和调试代码
              </p>
            ) : null}
            {chatMessages.map((m, i) => (
              <div
                key={i}
                className={`mb-3 rounded-lg px-3 py-2 text-sm whitespace-pre-wrap ${
                  m.role === 'user'
                    ? 'ml-8 bg-primary text-primary-foreground'
                    : 'mr-8 bg-muted'
                }`}
              >
                {m.content ||
                  (chatStreaming && i === chatMessages.length - 1 ? '思考中...' : '')}
              </div>
            ))}
          </ScrollArea>

          <form onSubmit={handleChatSubmit} className="border-t p-4">
            <div className="flex gap-2">
              <input
                type="text"
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                placeholder={hasApiKey ? '输入问题...' : '请先设置 API Key'}
                disabled={!hasApiKey || chatStreaming}
                className="flex-1 rounded border px-3 py-1.5 text-sm"
              />
              <Button type="submit" size="sm" disabled={!hasApiKey || chatStreaming}>
                发送
              </Button>
            </div>
          </form>
        </div>
      )}
    </>
  )
}
