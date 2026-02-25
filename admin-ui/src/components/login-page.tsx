import { useState } from 'react'
import { storage } from '@/lib/storage'
import { login } from '@/api/credentials'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { extractErrorMessage } from '@/lib/utils'

interface LoginPageProps {
  onLogin: () => void
}

export function LoginPage({ onLogin }: LoginPageProps) {
  const [username, setUsername] = useState('admin')
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!username.trim() || !password.trim()) return

    setLoading(true)
    setError('')
    try {
      const result = await login({ username: username.trim(), password: password.trim() })
      storage.setToken(result.token)
      onLogin()
    } catch (err) {
      setError(extractErrorMessage(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="relative flex min-h-screen items-center justify-center overflow-hidden bg-black p-4">
      <Card className="animate-fade-up w-full max-w-md border border-white/5 border-t-white/20 bg-black/50 backdrop-blur-2xl">
        <CardHeader className="animate-fade-up animate-fade-up-delay-1 space-y-3 text-center">
          <CardTitle className="font-mono text-2xl font-light tracking-[0.3em]">KIRO-RS</CardTitle>
          <CardDescription className="text-neutral-400">登录控制中心</CardDescription>
        </CardHeader>
        <CardContent className="animate-fade-up animate-fade-up-delay-2">
          <form onSubmit={handleSubmit} className="space-y-6">
            <Input
              type="text"
              placeholder="用户名"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="h-11 rounded-none border-x-0 border-t-0 border-b border-white/20 bg-transparent px-0 font-mono focus-visible:border-white/50"
            />
            <Input
              type="password"
              placeholder="密码"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="h-11 rounded-none border-x-0 border-t-0 border-b border-white/20 bg-transparent px-0 font-mono focus-visible:border-white/50"
            />
            {error && <div className="text-sm text-red-400">{error}</div>}
            <Button type="submit" className="w-full" disabled={loading || !username.trim() || !password.trim()}>
              {loading ? '登录中...' : '登录'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}
