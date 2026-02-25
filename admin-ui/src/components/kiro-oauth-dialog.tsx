import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { getCredentials } from '@/api/credentials'
import { extractErrorMessage } from '@/lib/utils'

interface KiroOAuthDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  baselineTotal: number
  onImported: () => void
}

export function KiroOAuthDialog({
  open,
  onOpenChange,
  baselineTotal,
  onImported,
}: KiroOAuthDialogProps) {
  const [idcStartUrl, setIdcStartUrl] = useState('')
  const [idcRegion, setIdcRegion] = useState('us-east-1')
  const [running, setRunning] = useState(false)
  const popupRef = useRef<Window | null>(null)
  const timerRef = useRef<number | null>(null)

  const stopPolling = () => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current)
      timerRef.current = null
    }
    setRunning(false)
  }

  const startPolling = (startTotal: number) => {
    stopPolling()
    setRunning(true)

    timerRef.current = window.setInterval(async () => {
      const popup = popupRef.current
      if (!popup || popup.closed) {
        stopPolling()
      }

      try {
        const latest = await getCredentials()
        if (latest.total > startTotal) {
          stopPolling()
          toast.success('OAuth 验证成功，凭据已导入')
          onImported()
          onOpenChange(false)
          popup?.close()
        }
      } catch (err) {
        toast.error(`检查 OAuth 状态失败: ${extractErrorMessage(err)}`)
        stopPolling()
      }
    }, 3000)
  }

  const openOAuthPopup = (url: string) => {
    const popup = window.open(
      url,
      'kiro-oauth',
      'width=980,height=820,menubar=no,toolbar=no,location=no,status=no,resizable=yes,scrollbars=yes'
    )
    if (!popup) {
      toast.error('浏览器拦截了弹窗，请允许弹窗后重试。')
      return
    }
    popupRef.current = popup
    startPolling(baselineTotal)
    toast.message('OAuth 流程已打开，授权完成后将自动导入凭据。')
  }

  const startBuilderId = () => {
    openOAuthPopup('/v0/oauth/kiro/start?method=builder-id')
  }

  const startIdc = () => {
    const startUrl = idcStartUrl.trim()
    if (!startUrl) {
      toast.error('请输入 IDC Start URL')
      return
    }
    const region = idcRegion.trim() || 'us-east-1'
    openOAuthPopup(
      `/v0/oauth/kiro/start?method=idc&startUrl=${encodeURIComponent(startUrl)}&region=${encodeURIComponent(region)}`
    )
  }

  useEffect(() => {
    if (!open) {
      stopPolling()
    }
    return () => stopPolling()
  }, [open])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-center font-mono text-sm tracking-normal text-neutral-400">
            Kiro OAuth 导入
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-6 py-2">
          <div className="flex min-h-[160px] flex-col items-center justify-center rounded-xl border border-white/10 bg-black/40 text-center">
            <div className="mb-2 scale-125">
              <div className="orbital-loader" />
            </div>
            <p className="animate-pulse text-sm text-neutral-400">正在与 Kiro 进行身份验证...</p>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <div className="space-y-3 rounded-xl border border-white/10 bg-black/40 p-4">
              <div className="font-mono text-xs uppercase tracking-[0.2em] text-neutral-500">AWS Builder ID</div>
              <p className="text-xs text-neutral-400">推荐个人账号使用 OAuth 弹窗登录。</p>
              <Button onClick={startBuilderId} disabled={running} className="w-full">
                启动 Builder ID 登录
              </Button>
            </div>

            <div className="space-y-3 rounded-xl border border-white/10 bg-black/40 p-4">
              <div className="font-mono text-xs uppercase tracking-[0.2em] text-neutral-500">AWS IDC</div>
              <Input
                placeholder="https://your-org.awsapps.com/start"
                value={idcStartUrl}
                onChange={(e) => setIdcStartUrl(e.target.value)}
                disabled={running}
              />
              <Input
                placeholder="us-east-1"
                value={idcRegion}
                onChange={(e) => setIdcRegion(e.target.value)}
                disabled={running}
              />
              <Button variant="secondary" onClick={startIdc} disabled={running} className="w-full">
                启动 IDC 登录
              </Button>
            </div>
          </div>

          <div className="rounded-xl border border-white/10 bg-black/30 px-4 py-3 text-xs text-neutral-400">
            {running
              ? '等待 OAuth 完成，导入后凭据列表会自动刷新。'
              : 'OAuth 当前空闲。你也可以访问 /v0/oauth/kiro 手动导入。'}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
