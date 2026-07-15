import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { getCaptcha, login } from '@/lib/api'
import { useAuth } from '@/stores/auth'

export default function LoginPage() {
  const navigate = useNavigate()
  const { login: authLogin } = useAuth()

  const [stuId, setStuId] = useState('')
  const [password, setPassword] = useState('')
  const [captchaCode, setCaptchaCode] = useState('')
  const [captchaImage, setCaptchaImage] = useState('')
  const [sessionId, setSessionId] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  async function fetchCaptcha() {
    setError('')
    setCaptchaCode('')
    try {
      const data = await getCaptcha()
      setSessionId(data.session_id)
      setCaptchaImage(data.captcha_image)
    } catch {
      setError('获取验证码失败')
    }
  }

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault()
    setError('')
    setLoading(true)
    try {
      const data = await login(stuId, password, captchaCode, sessionId)
      authLogin(data.token)
      navigate('/courses')
    } catch (err) {
      setError(err instanceof Error ? err.message : '登录失败')
      fetchCaptcha() // refresh captcha on failure
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchCaptcha()
  }, [])

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle className="text-center">CG 系统登录</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleLogin} className="space-y-4">
            <div>
              <Input
                type="text"
                placeholder="学号"
                value={stuId}
                onChange={(e) => setStuId(e.target.value)}
                required
              />
            </div>
            <div>
              <Input
                type="password"
                placeholder="密码"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
              />
            </div>
            <div className="flex gap-2">
              <Input
                type="text"
                placeholder="验证码"
                value={captchaCode}
                onChange={(e) => setCaptchaCode(e.target.value)}
                required
                className="flex-1"
              />
              {captchaImage && (
                <img
                  src={`data:image/png;base64,${captchaImage}`}
                  alt="验证码"
                  className="h-10 cursor-pointer rounded border"
                  onClick={fetchCaptcha}
                  title="点击刷新验证码"
                />
              )}
            </div>
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" className="w-full" disabled={loading}>
              {loading ? '登录中...' : '登录'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}
