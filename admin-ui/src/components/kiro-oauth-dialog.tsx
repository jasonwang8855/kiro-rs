import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { ExternalLink, Loader2, CheckCircle2, XCircle, Copy } from 'lucide-react'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { extractErrorMessage, copyToClipboard } from '@/lib/utils'

interface OAuthStartResponse {
  stateId: string
  userCode: string
  verificationUri: string
  expiresIn: number
}

interface OAuthStatusResponse {
  status: 'pending' | 'success' | 'failed'
  remaining_seconds?: number
  credential_id?: number
  error?: string
}

interface KiroOAuthDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  baselineTotal?: number
  onImported: () => void
}

type Phase = 'select' | 'loading' | 'verify' | 'success' | 'failed'

export function KiroOAuthDialog({
  open,
  onOpenChange,
  onImported,
}: KiroOAuthDialogProps) {
  const [idcStartUrl, setIdcStartUrl] = useState('')
  const [idcRegion, setIdcRegion] = useState('us-east-1')
  const [phase, setPhase] = useState<Phase>('select')
  const [authData, setAuthData] = useState<OAuthStartResponse | null>(null)
  const [remaining, setRemaining] = useState(0)
  const [errorMsg, setErrorMsg] = useState('')
  const timerRef = useRef<number | null>(null)

  const stopPolling = () => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current)
      timerRef.current = null
    }
  }

  const reset = () => {
    stopPolling()
    setPhase('select')
    setAuthData(null)
    setRemaining(0)
    setErrorMsg('')
  }

  const startPolling = (stateId: string) => {
    stopPolling()
    timerRef.current = window.setInterval(async () => {
      try {
        const resp = await fetch(`/v0/oauth/kiro/status?state=${encodeURIComponent(stateId)}`)
        const data: OAuthStatusResponse = await resp.json()
        if (data.status === 'success') {
          stopPolling()
          setPhase('success')
          toast.success('OAuth 验证成功，凭据已导入')
          onImported()
        } else if (data.status === 'failed') {
          stopPolling()
          setPhase('failed')
          setErrorMsg(data.error || '未知错误')
        } else if (data.remaining_seconds !== undefined) {
          setRemaining(data.remaining_seconds)
        }
      } catch (err) {
        stopPolling()
        setPhase('failed')
        setErrorMsg(extractErrorMessage(err))
      }
    }, 3000)
  }

  const startOAuth = async (method: string, startUrl?: string, region?: string) => {
    setPhase('loading')
    setErrorMsg('')
    try {
      const body: Record<string, string> = { method }
      if (startUrl) body.startUrl = startUrl
      if (region) body.region = region

      const resp = await fetch('/v0/oauth/kiro/start-json', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      const data = await resp.json()
      if (!resp.ok) {
        setPhase('failed')
        setErrorMsg(data.error || `HTTP ${resp.status}`)
        return
      }
      setAuthData(data as OAuthStartResponse)
      setRemaining(data.expiresIn)
      setPhase('verify')
      startPolling(data.stateId)

      // 自动打开授权页面，无需用户二次点击
      window.open(data.verificationUri, '_blank', 'noopener,noreferrer')
    } catch (err) {
      setPhase('failed')
      setErrorMsg(extractErrorMessage(err))
    }
  }

  const handleCopyCode = async () => {
    if (!authData) return
    try {
      await copyToClipboard(authData.userCode)
      toast.success('验证码已复制')
    } catch {
      toast.error('复制失败')
    }
  }

  useEffect(() => {
    if (!open) reset()
    return () => stopPolling()
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-center font-mono text-sm tracking-normal text-neutral-400">
            Kiro OAuth 导入
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-2">
          {phase === 'select' && (
            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-3 rounded-xl border border-white/10 bg-black/40 p-4">
                <div className="font-mono text-xs uppercase tracking-[0.2em] text-neutral-500">
                  AWS Builder ID
                </div>
                <p className="text-xs text-neutral-400">推荐个人账号使用。</p>
                <Button onClick={() => startOAuth('builder-id')} className="w-full">
                  启动 Builder ID 登录
                </Button>
              </div>
              <div className="space-y-3 rounded-xl border border-white/10 bg-black/40 p-4">
                <div className="font-mono text-xs uppercase tracking-[0.2em] text-neutral-500">
                  AWS IDC
                </div>
                <Input
                  placeholder="https://your-org.awsapps.com/start"
                  value={idcStartUrl}
                  onChange={(e) => setIdcStartUrl(e.target.value)}
                />
                <Input
                  placeholder="us-east-1"
                  value={idcRegion}
                  onChange={(e) => setIdcRegion(e.target.value)}
                />
                <Button
                  variant="secondary"
                  onClick={() => {
                    const url = idcStartUrl.trim()
                    if (!url) { toast.error('请输入 IDC Start URL'); return }
                    startOAuth('idc', url, idcRegion.trim() || 'us-east-1')
                  }}
                  className="w-full"
                >
                  启动 IDC 登录
                </Button>
              </div>
            </div>
          )}

          {phase === 'loading' && (
            <div className="flex min-h-[160px] flex-col items-center justify-center gap-3 rounded-xl border border-white/10 bg-black/40">
              <Loader2 className="h-6 w-6 animate-spin text-neutral-400" />
              <p className="text-sm text-neutral-400">正在初始化 OAuth 设备授权...</p>
            </div>
          )}

          {phase === 'verify' && authData && (
            <div className="space-y-4">
              <div className="rounded-xl border border-white/10 bg-black/40 p-5 text-center space-y-3">
                <p className="text-xs font-mono uppercase tracking-[0.2em] text-neutral-500">
                  授权页面已打开 · 请在浏览器中完成登录
                </p>
                <p className="text-xs text-neutral-500">如果页面未自动打开，请点击下方链接</p>
                <a
                  href={authData.verificationUri}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 rounded-lg bg-white/10 px-4 py-2 text-sm text-white hover:bg-white/15 transition-colors"
                >
                  手动打开验证页面
                  <ExternalLink className="h-4 w-4" />
                </a>
              </div>
              <div className="rounded-xl border border-white/10 bg-black/40 p-5 text-center space-y-3">
                <p className="text-xs font-mono uppercase tracking-[0.2em] text-neutral-500">
                  验证码（如需手动输入）
                </p>
                <div className="flex items-center justify-center gap-3">
                  <span className="font-mono text-3xl font-bold tracking-[6px] text-white">
                    {authData.userCode}
                  </span>
                  <button
                    onClick={handleCopyCode}
                    className="rounded-md p-1.5 text-neutral-400 hover:bg-white/10 hover:text-white transition-colors"
                  >
                    <Copy className="h-4 w-4" />
                  </button>
                </div>
              </div>
              <div className="rounded-xl border border-white/10 bg-black/40 p-4 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Loader2 className="h-4 w-4 animate-spin text-neutral-500" />
                  <span className="text-xs text-neutral-400">等待授权完成</span>
                </div>
                <span className="font-mono text-xs text-neutral-500">{remaining}s</span>
              </div>
            </div>
          )}

          {phase === 'success' && (
            <div className="flex min-h-[160px] flex-col items-center justify-center gap-3 rounded-xl border border-white/10 bg-black/40">
              <CheckCircle2 className="h-8 w-8 text-emerald-400" />
              <p className="text-sm text-neutral-200">验证成功，凭据已导入</p>
              <Button variant="secondary" size="sm" onClick={() => onOpenChange(false)}>
                关闭
              </Button>
            </div>
          )}

          {phase === 'failed' && (
            <div className="flex min-h-[160px] flex-col items-center justify-center gap-3 rounded-xl border border-white/10 bg-black/40 px-4">
              <XCircle className="h-8 w-8 text-red-400" />
              <p className="text-sm text-red-300 text-center">{errorMsg}</p>
              <Button variant="secondary" size="sm" onClick={reset}>
                重试
              </Button>
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

