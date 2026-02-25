import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'

export interface VerifyResult {
  id: number
  status: 'pending' | 'verifying' | 'success' | 'failed'
  usage?: string
  error?: string
}

interface BatchVerifyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  verifying: boolean
  progress: { current: number; total: number }
  results: Map<number, VerifyResult>
  onCancel: () => void
}

export function BatchVerifyDialog({
  open,
  onOpenChange,
  verifying,
  progress,
  results,
  onCancel,
}: BatchVerifyDialogProps) {
  const resultsArray = Array.from(results.values())
  const successCount = resultsArray.filter((r) => r.status === 'success').length
  const failedCount = resultsArray.filter((r) => r.status === 'failed').length

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="font-mono text-sm tracking-normal text-neutral-400">
            批量验证
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {verifying && (
            <div className="space-y-2">
              <div className="flex justify-between text-sm font-mono text-neutral-400">
                <span>进度</span>
                <span>
                  {progress.current} / {progress.total}
                </span>
              </div>
              <div className="h-2 w-full rounded-full bg-white/10">
                <div
                  className="h-2 rounded-full bg-gradient-to-r from-neutral-500 to-white transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]"
                  style={{ width: `${(progress.current / progress.total) * 100}%` }}
                />
              </div>
            </div>
          )}

          {results.size > 0 && (
            <div className="flex justify-between text-sm font-mono text-neutral-400">
              <span>结果</span>
              <span>
                成功: {successCount} / 失败: {failedCount}
              </span>
            </div>
          )}

          {results.size > 0 && (
            <div className="max-h-[400px] space-y-1 overflow-y-auto rounded-md border border-white/10 bg-black/30 p-2">
              {resultsArray.map((result) => (
                <div
                  key={result.id}
                  className={`rounded border px-3 py-2 text-sm ${
                    result.status === 'success'
                      ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
                      : result.status === 'failed'
                        ? 'border-red-500/30 bg-red-500/10 text-red-300'
                        : result.status === 'verifying'
                          ? 'border-amber-500/30 bg-amber-500/10 text-amber-300'
                          : 'border-white/10 bg-white/5 text-neutral-300'
                  }`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <span className="font-mono">凭据 #{result.id}</span>
                      {result.status === 'success' && result.usage && (
                        <Badge variant="outline" className="text-xs">
                          {result.usage}
                        </Badge>
                      )}
                    </div>
                    <span>
                      {result.status === 'success' && '成功'}
                      {result.status === 'failed' && '失败'}
                      {result.status === 'verifying' && <div className="orbital-loader scale-75" />}
                      {result.status === 'pending' && '等待'}
                    </span>
                  </div>
                  {result.error && <div className="mt-1 text-xs opacity-90">错误: {result.error}</div>}
                </div>
              ))}
            </div>
          )}

          {verifying && (
            <p className="text-xs text-neutral-400">
              验证会在请求间隔下执行。你可以关闭此对话框并在后台继续运行。
            </p>
          )}
        </div>

        <div className="flex justify-end gap-2 border-t border-white/10 pt-4">
          {verifying ? (
            <>
              <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
                后台运行
              </Button>
              <Button type="button" variant="destructive" onClick={onCancel}>
                取消验证
              </Button>
            </>
          ) : (
            <Button type="button" onClick={() => onOpenChange(false)}>
              关闭
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
