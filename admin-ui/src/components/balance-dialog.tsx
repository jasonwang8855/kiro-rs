import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Progress } from '@/components/ui/progress'
import { useCredentialBalance } from '@/hooks/use-credentials'
import { parseError } from '@/lib/utils'

interface BalanceDialogProps {
  credentialId: number | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function BalanceDialog({ credentialId, open, onOpenChange }: BalanceDialogProps) {
  const { data: balance, isLoading, error } = useCredentialBalance(credentialId)

  const formatDate = (timestamp: number | null) => {
    if (!timestamp) return '未知'
    return new Date(timestamp * 1000).toLocaleString('zh-CN')
  }

  const formatNumber = (num: number) => {
    return num.toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="font-mono text-sm tracking-normal text-neutral-400">
            凭据 #{credentialId} 余额
          </DialogTitle>
        </DialogHeader>

        {isLoading && (
          <div className="flex items-center justify-center py-10">
            <div className="orbital-loader" />
          </div>
        )}

        {error && (() => {
          const parsed = parseError(error)
          return (
            <div className="space-y-3 py-6">
              <div className="flex items-center justify-center gap-2 text-red-400">
                <svg className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                  <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clipRule="evenodd" />
                </svg>
                <span className="font-medium">{parsed.title}</span>
              </div>
              {parsed.detail && <div className="px-4 text-center text-sm text-neutral-500">{parsed.detail}</div>}
            </div>
          )
        })()}

        {balance && (
          <div className="space-y-6">
            <div className="text-center">
              <div className="text-xs tracking-normal text-neutral-400">订阅计划</div>
              <div className="mt-2 text-lg font-mono text-white">{balance.subscriptionTitle || '未知'}</div>
            </div>

            <div className="space-y-3">
              <div className="text-center text-5xl font-mono font-light text-white">${formatNumber(balance.remaining)}</div>
              <Progress value={balance.usagePercentage} />
              <div className="flex justify-between text-xs font-mono text-neutral-400">
                <span>已用 ${formatNumber(balance.currentUsage)}</span>
                <span>限额 ${formatNumber(balance.usageLimit)}</span>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-2 border-t border-white/10 pt-4 text-sm font-mono text-neutral-400">
              <div className="flex items-center justify-between gap-2">
                <span>额度使用</span>
                <span>{balance.usagePercentage.toFixed(1)}%</span>
              </div>
              <div className="flex items-center justify-between gap-2">
                <span>下次重置</span>
                <span>{formatDate(balance.nextResetAt)}</span>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
