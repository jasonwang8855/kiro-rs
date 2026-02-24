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
          toast.success('OAuth 验证成功，凭证已自动导入')
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
      toast.error('浏览器拦截了弹窗，请允许弹窗后重试')
      return
    }
    popupRef.current = popup
    startPolling(baselineTotal)
    toast.message('OAuth 流程已打开，完成授权后将自动导入')
  }

  const startBuilderId = () => {
    openOAuthPopup('/v0/oauth/kiro/start?method=builder-id')
  }

  const startIdc = () => {
    const startUrl = idcStartUrl.trim()
    if (!startUrl) {
      toast.error('请先填写 IDC Start URL')
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
      <DialogContent className="sm:max-w-2xl border-slate-200 bg-white/95 backdrop-blur">
        <DialogHeader>
          <DialogTitle className="text-xl">Kiro OAuth 快速导入</DialogTitle>
        </DialogHeader>

        <div className="grid gap-4 md:grid-cols-2">
          <div className="rounded-xl border border-amber-200 bg-gradient-to-br from-amber-50 to-orange-50 p-4 space-y-3">
            <div className="text-sm font-semibold text-amber-900">AWS Builder ID</div>
            <p className="text-xs text-amber-800">
              推荐个人账号使用。点击后会打开 OAuth 页面，验证成功自动导入凭证。
            </p>
            <Button onClick={startBuilderId} disabled={running} className="w-full">
              开始 Builder ID 登录
            </Button>
          </div>

          <div className="rounded-xl border border-sky-200 bg-gradient-to-br from-sky-50 to-cyan-50 p-4 space-y-3">
            <div className="text-sm font-semibold text-sky-900">AWS IDC</div>
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
              开始 IDC 登录
            </Button>
          </div>
        </div>

        <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-xs text-slate-600">
          {running
            ? '正在等待 OAuth 验证完成，导入后会自动刷新凭证列表。'
            : '未启动 OAuth。你也可以直接访问 /v0/oauth/kiro 页面进行手动导入。'}
        </div>
      </DialogContent>
    </Dialog>
  )
}
