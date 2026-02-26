import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { SuccessCheck } from '@/components/ui/success-check'
import { useAddCredential } from '@/hooks/use-credentials'
import { extractErrorMessage } from '@/lib/utils'

interface AddCredentialDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

type AuthMethod = 'social' | 'idc'

export function AddCredentialDialog({ open, onOpenChange }: AddCredentialDialogProps) {
  const [refreshToken, setRefreshToken] = useState('')
  const [authMethod, setAuthMethod] = useState<AuthMethod>('social')
  const [authRegion, setAuthRegion] = useState('')
  const [apiRegion, setApiRegion] = useState('')
  const [clientId, setClientId] = useState('')
  const [clientSecret, setClientSecret] = useState('')
  const [priority, setPriority] = useState('0')
  const [machineId, setMachineId] = useState('')
  const [proxyUrl, setProxyUrl] = useState('')
  const [proxyUsername, setProxyUsername] = useState('')
  const [proxyPassword, setProxyPassword] = useState('')
  const [showSuccess, setShowSuccess] = useState(false)

  const successTimerRef = useRef<number | null>(null)
  const { mutate, isPending } = useAddCredential()

  const resetForm = () => {
    setRefreshToken('')
    setAuthMethod('social')
    setAuthRegion('')
    setApiRegion('')
    setClientId('')
    setClientSecret('')
    setPriority('0')
    setMachineId('')
    setProxyUrl('')
    setProxyUsername('')
    setProxyPassword('')
  }

  useEffect(() => {
    if (!open) {
      setShowSuccess(false)
      if (successTimerRef.current !== null) {
        window.clearTimeout(successTimerRef.current)
        successTimerRef.current = null
      }
    }

    return () => {
      if (successTimerRef.current !== null) {
        window.clearTimeout(successTimerRef.current)
        successTimerRef.current = null
      }
    }
  }, [open])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!refreshToken.trim()) {
      toast.error('请输入刷新令牌')
      return
    }

    if (authMethod === 'idc' && (!clientId.trim() || !clientSecret.trim())) {
      toast.error('IdC/Builder-ID/IAM 方式必须填写 Client ID 和 Client Secret')
      return
    }

    mutate(
      {
        refreshToken: refreshToken.trim(),
        authMethod,
        authRegion: authRegion.trim() || undefined,
        apiRegion: apiRegion.trim() || undefined,
        clientId: clientId.trim() || undefined,
        clientSecret: clientSecret.trim() || undefined,
        priority: parseInt(priority) || 0,
        machineId: machineId.trim() || undefined,
        proxyUrl: proxyUrl.trim() || undefined,
        proxyUsername: proxyUsername.trim() || undefined,
        proxyPassword: proxyPassword.trim() || undefined,
      },
      {
        onSuccess: (data) => {
          toast.success(data.message)
          setShowSuccess(true)

          if (successTimerRef.current !== null) {
            window.clearTimeout(successTimerRef.current)
          }
          successTimerRef.current = window.setTimeout(() => {
            setShowSuccess(false)
            onOpenChange(false)
            resetForm()
            successTimerRef.current = null
          }, 1000)
        },
        onError: (error: unknown) => {
          toast.error(`添加失败: ${extractErrorMessage(error)}`)
        },
      }
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="font-mono text-sm tracking-normal text-neutral-400">
            添加凭据
          </DialogTitle>
        </DialogHeader>

        {showSuccess ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-3 py-10">
            <SuccessCheck size={64} />
            <p className="font-mono text-sm text-neutral-300">凭据添加成功</p>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="flex min-h-0 flex-1 flex-col">
            <div className="flex-1 space-y-5 overflow-y-auto py-4 pr-1">
              <div className="space-y-2">
                <label htmlFor="refreshToken" className="font-mono text-xs tracking-normal text-neutral-400">
                  刷新令牌 <span className="text-red-400">*</span>
                </label>
                <Input
                  id="refreshToken"
                  type="password"
                  placeholder="请输入刷新令牌"
                  value={refreshToken}
                  onChange={(e) => setRefreshToken(e.target.value)}
                  disabled={isPending}
                />
              </div>

              <div className="space-y-2">
                <label htmlFor="authMethod" className="font-mono text-xs tracking-normal text-neutral-400">
                  认证方式
                </label>
                <select
                  id="authMethod"
                  value={authMethod}
                  onChange={(e) => setAuthMethod(e.target.value as AuthMethod)}
                  disabled={isPending}
                  className="h-11 w-full rounded-md border border-white/15 bg-transparent px-3 py-2 font-mono text-sm text-white ring-offset-background focus-visible:border-white/50 focus-visible:outline-none focus-visible:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <option value="social" className="bg-black text-white">Social</option>
                  <option value="idc" className="bg-black text-white">IdC/Builder-ID/IAM</option>
                </select>
              </div>

              <div className="space-y-2">
                <label className="font-mono text-xs tracking-normal text-neutral-400">区域设置</label>
                <div className="grid grid-cols-1 gap-2">
                  <Input
                    id="authRegion"
                    placeholder="认证区域"
                    value={authRegion}
                    onChange={(e) => setAuthRegion(e.target.value)}
                    disabled={isPending}
                  />
                  <Input
                    id="apiRegion"
                    placeholder="API 区域"
                    value={apiRegion}
                    onChange={(e) => setApiRegion(e.target.value)}
                    disabled={isPending}
                  />
                </div>
                <p className="text-xs text-neutral-400">留空则使用全局配置默认值。</p>
              </div>

              {authMethod === 'idc' && (
                <>
                  <div className="space-y-2">
                    <label htmlFor="clientId" className="font-mono text-xs tracking-[0.2em] text-neutral-500">
                      Client ID <span className="text-red-400">*</span>
                    </label>
                    <Input
                      id="clientId"
                      placeholder="请输入 Client ID"
                      value={clientId}
                      onChange={(e) => setClientId(e.target.value)}
                      disabled={isPending}
                    />
                  </div>
                  <div className="space-y-2">
                    <label htmlFor="clientSecret" className="font-mono text-xs tracking-[0.2em] text-neutral-500">
                      Client Secret <span className="text-red-400">*</span>
                    </label>
                    <Input
                      id="clientSecret"
                      type="password"
                      placeholder="请输入 Client Secret"
                      value={clientSecret}
                      onChange={(e) => setClientSecret(e.target.value)}
                      disabled={isPending}
                    />
                  </div>
                </>
              )}

              <div className="space-y-2">
                <label htmlFor="priority" className="font-mono text-xs tracking-normal text-neutral-400">
                  优先级
                </label>
                <Input
                  id="priority"
                  type="number"
                  min="0"
                  placeholder="数值越小优先级越高"
                  value={priority}
                  onChange={(e) => setPriority(e.target.value)}
                  disabled={isPending}
                />
                <p className="text-xs text-neutral-400">默认值为 0。</p>
              </div>

              <div className="space-y-2">
                <label htmlFor="machineId" className="font-mono text-xs tracking-normal text-neutral-400">
                  机器 ID
                </label>
                <Input
                  id="machineId"
                  placeholder="可选 64 位十六进制，留空自动生成"
                  value={machineId}
                  onChange={(e) => setMachineId(e.target.value)}
                  disabled={isPending}
                />
              </div>

              <div className="space-y-2">
                <label className="font-mono text-xs tracking-normal text-neutral-400">代理</label>
                <Input
                  id="proxyUrl"
                  placeholder="代理 URL 或直连"
                  value={proxyUrl}
                  onChange={(e) => setProxyUrl(e.target.value)}
                  disabled={isPending}
                />
                <div className="grid grid-cols-1 gap-2">
                  <Input
                    id="proxyUsername"
                    placeholder="代理用户名"
                    value={proxyUsername}
                    onChange={(e) => setProxyUsername(e.target.value)}
                    disabled={isPending}
                  />
                  <Input
                    id="proxyPassword"
                    type="password"
                    placeholder="代理密码"
                    value={proxyPassword}
                    onChange={(e) => setProxyPassword(e.target.value)}
                    disabled={isPending}
                  />
                </div>
              </div>
            </div>

            <DialogFooter className="mt-4 border-t border-white/10 pt-4">
              <Button
                type="button"
                variant="secondary"
                onClick={() => onOpenChange(false)}
                disabled={isPending}
              >
                取消
              </Button>
              <Button type="submit" disabled={isPending}>
                {isPending ? (
                  <span className="inline-flex items-center gap-2">
                    <div className="orbital-loader scale-75" />
                    创建中...
                  </span>
                ) : (
                  '创建'
                )}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  )
}
