import { createContext, useContext, useState, type ReactNode } from 'react'

/** 当前打开的题目上下文，供 AI 面板作为对话背景 */
interface ProblemContextValue {
  /** 题面纯文本（statement_text），无题目时为 null */
  statementText: string | null
  /** 题目标题/标识 */
  title: string | null
  setProblem: (statementText: string, title: string) => void
  clearProblem: () => void
}

const ProblemContext = createContext<ProblemContextValue>({
  statementText: null,
  title: null,
  setProblem: () => {},
  clearProblem: () => {},
})

export function ProblemContextProvider({ children }: { children: ReactNode }) {
  const [statementText, setStatementText] = useState<string | null>(null)
  const [title, setTitle] = useState<string | null>(null)

  return (
    <ProblemContext.Provider
      value={{
        statementText,
        title,
        setProblem: (text, t) => {
          setStatementText(text)
          setTitle(t)
        },
        clearProblem: () => {
          setStatementText(null)
          setTitle(null)
        },
      }}
    >
      {children}
    </ProblemContext.Provider>
  )
}

export function useProblemContext() {
  return useContext(ProblemContext)
}
