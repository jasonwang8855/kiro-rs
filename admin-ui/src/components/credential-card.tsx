import { useState } from 'react'
import { toast } from 'sonner'
import { RefreshCw, ChevronUp, ChevronDown, Wallet, Trash2 } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { cn } from '@/lib/utils'
import type { CredentialStatusItem, BalanceResponse } from '@/types/api'
import {
  useSetDisabled,
  useSetPriority,
  useResetFailure,
  useDeleteCredential,
} from '@/hooks/use-credentials'

interface CredentialCardProps {
  credential: CredentialStatusItem
  onViewBalance: (id: number) => void
  selected: boolean
  onToggleSelect: () => void
  balance: BalanceResponse | null
  loadingBalance: boolean
}

function formatLastUsed(lastUsedAt: string | null): string {
  if (!lastUsedAt) return '从未使用'
  const date = new Date(lastUsedAt)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  if (diff < 0) return '刚刚'
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds} 秒前`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes} 分钟前`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  return `${days} 天前`
}

function getStatusMeta(credential: CredentialStatusItem) {
  if (credential.disabled) {
    return {
      led: 'status-led-error',
      label: '已禁用',
    }
  }
  if (credential.failureCount >= 3) {
    return {
      led: 'status-led-warn',
      label: '已限流',
    }
  }
  return {
    led: 'status-led-active',
    label: '运行中',
  }
}

export function CredentialCard({
  credential,
  onViewBalance,
  selected,
  onToggleSelect,
  balance,
  loadingBalance,
}: CredentialCardProps) {
  const [editingPriority, setEditingPriority] = useState(false)
  const [priorityValue, setPriorityValue] = useState(String(credential.priority))
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)

  const setDisabled = useSetDisabled()
  const setPriority = useSetPriority()
  const resetFailure = useResetFailure()
  const deleteCredential = useDeleteCredential()

  const statusMeta = getStatusMeta(credential)

  const handleToggleDisabled = () => {
    setDisabled.mutate(
      { id: credential.id, disabled: !credential.disabled },
      {
        onSuccess: (res) => {
          toast.success(res.message)
        },
        onError: (err) => {
          toast.error('操作失败: ' + (err as Error).message)
        },
      }
    )
  }

  const handlePriorityChange = () => {
    const newPriority = parseInt(priorityValue, 10)
    if (isNaN(newPriority) || newPriority < 0) {
      toast.error('优先级必须是非负整数')
      return
    }
    setPriority.mutate(
      { id: credential.id, priority: newPriority },
      {
        onSuccess: (res) => {
          toast.success(res.message)
          setEditingPriority(false)
        },
        onError: (err) => {
          toast.error('操作失败: ' + (err as Error).message)
        },
      }
    )
  }

  const handleReset = () => {
    resetFailure.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
      },
      onError: (err) => {
        toast.error('操作失败: ' + (err as Error).message)
      },
    })
  }

  const handleDelete = () => {
    if (!credential.disabled) {
      toast.error('请先禁用凭据再删除')
      setShowDeleteDialog(false)
      return
    }

    deleteCredential.mutate(credential.id, {
      onSuccess: (res) => {
        toast.success(res.message)
        setShowDeleteDialog(false)
      },
      onError: (err) => {
        toast.error('删除失败: ' + (err as Error).message)
      },
    })
  }

  return (
    <>
      <Card
        className={cn(
          'flex h-full flex-col',
          credential.isCurrent && 'ring-1 ring-white/40',
          credential.disabled && 'opacity-80'
        )}
      >
        <CardHeader className="space-y-3 pb-0">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <Checkbox checked={selected} onCheckedChange={onToggleSelect} />
              <Badge variant="secondary">{credential.authMethod || '未知'}</Badge>
              {credential.isCurrent && <Badge variant="default">当前使用</Badge>}
            </div>
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center gap-2 text-xs font-mono text-neutral-400">
                <div className={statusMeta.led} aria-hidden="true" />
                {statusMeta.label}
              </span>
              <Switch
                checked={!credential.disabled}
                onCheckedChange={handleToggleDisabled}
                disabled={setDisabled.isPending}
              />
            </div>
          </div>

          <CardTitle className="font-mono text-xl font-normal tracking-tight text-white">
            {credential.email || `credential-${credential.id}`}
          </CardTitle>
          <div className="font-mono text-xs text-neutral-500">ID #{credential.id}</div>
        </CardHeader>

        <CardContent className="mt-4 flex flex-1 flex-col gap-4">
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div>
              <div className="text-[11px] font-sans font-medium tracking-wide text-neutral-500">优先级</div>
              {editingPriority ? (
                <div className="mt-1 inline-flex items-center gap-1">
                  <Input
                    type="number"
                    value={priorityValue}
                    onChange={(e) => setPriorityValue(e.target.value)}
                    className="h-8 w-20"
                    min="0"
                  />
                  <Button
                    size="sm"
                    variant="secondary"
                    className="h-8 px-2"
                    onClick={handlePriorityChange}
                    disabled={setPriority.isPending}
                  >
                    保存
                  </Button>
                  <Button
                    size="sm"
                    variant="secondary"
                    className="h-8 px-2"
                    onClick={() => {
                      setEditingPriority(false)
                      setPriorityValue(String(credential.priority))
                    }}
                  >
                    取消
                  </Button>
                </div>
              ) : (
                <button
                  type="button"
                  className="mt-1 font-mono text-sm text-white/90 hover:text-white"
                  onClick={() => setEditingPriority(true)}
                >
                  {credential.priority}
                </button>
              )}
            </div>

            <div>
              <div className="text-[11px] font-sans font-medium tracking-wide text-neutral-500">失败次数</div>
              <div className={cn('mt-1 font-mono text-sm', credential.failureCount > 0 ? 'text-red-400' : 'text-white')}>
                {credential.failureCount}
              </div>
            </div>

            <div>
              <div className="text-[11px] font-sans font-medium tracking-wide text-neutral-500">成功次数</div>
              <div className="mt-1 font-mono text-sm text-white">{credential.successCount}</div>
            </div>

            <div>
              <div className="text-[11px] font-sans font-medium tracking-wide text-neutral-500">订阅计划</div>
              <div className="mt-1 font-mono text-sm text-white">
                {loadingBalance ? <div className="orbital-loader scale-75" /> : balance?.subscriptionTitle || '未知'}
              </div>
            </div>
          </div>

          <div className="space-y-2 border-t border-white/10 pt-3 text-xs font-mono text-neutral-400">
            <div className="flex items-center justify-between gap-3">
              <span className="font-sans font-medium tracking-wide text-neutral-500 text-[11px]">额度使用</span>
              {loadingBalance ? (
                <span className="inline-flex items-center gap-1">
                  <div className="orbital-loader scale-75" />
                </span>
              ) : balance ? (
                <span className="text-white">{balance.remaining.toFixed(2)} <span className="text-neutral-600">/</span> {balance.usageLimit.toFixed(2)}</span>
              ) : (
                <span className="font-sans text-[11px] text-neutral-500">未知</span>
              )}
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="font-sans font-medium tracking-wide text-neutral-500 text-[11px]">上次使用</span>
              <span className="text-white font-sans text-[11px]">{formatLastUsed(credential.lastUsedAt)}</span>
            </div>
            {credential.hasProxy && (
              <div className="flex items-center justify-between gap-3">
                <span className="font-sans font-medium tracking-wide text-neutral-500 text-[11px]">代理</span>
                <span className="truncate text-white">{credential.proxyUrl}</span>
              </div>
            )}
            {credential.hasProfileArn && (
              <div>
                <Badge variant="outline">Profile ARN</Badge>
              </div>
            )}
          </div>

          <div className="mt-auto flex flex-wrap items-center justify-between border-t border-white/10 pt-3">
            <div className="flex gap-1">
              <Button
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-neutral-400 hover:text-white"
                onClick={handleReset}
                disabled={resetFailure.isPending || credential.failureCount === 0}
                title="重置失败"
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-neutral-400 hover:text-white"
                onClick={() => {
                  const newPriority = Math.max(0, credential.priority - 1)
                  setPriority.mutate(
                    { id: credential.id, priority: newPriority },
                    {
                      onSuccess: (res) => toast.success(res.message),
                      onError: (err) => toast.error('操作失败: ' + (err as Error).message),
                    }
                  )
                }}
                disabled={setPriority.isPending || credential.priority === 0}
                title="提高优先级"
              >
                <ChevronUp className="h-4 w-4" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                className="h-8 w-8 text-neutral-400 hover:text-white"
                onClick={() => {
                  const newPriority = credential.priority + 1
                  setPriority.mutate(
                    { id: credential.id, priority: newPriority },
                    {
                      onSuccess: (res) => toast.success(res.message),
                      onError: (err) => toast.error('操作失败: ' + (err as Error).message),
                    }
                  )
                }}
                disabled={setPriority.isPending}
                title="降低优先级"
              >
                <ChevronDown className="h-4 w-4" />
              </Button>
            </div>
            
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" className="h-8 bg-transparent text-neutral-300 hover:text-white" onClick={() => onViewBalance(credential.id)}>
                <Wallet className="mr-1 h-3 w-3" />
                查询余额
              </Button>
              <Button
                size="sm"
                variant="destructive"
                className="h-8 bg-transparent border border-red-500/30 text-red-400 hover:bg-red-500/10 hover:text-red-300"
                onClick={() => setShowDeleteDialog(true)}
                disabled={!credential.disabled}
                title={!credential.disabled ? '请先禁用凭据再删除' : undefined}
              >
                <Trash2 className="mr-1 h-3 w-3" />
                删除
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>删除凭据</DialogTitle>
            <DialogDescription>
              确认删除凭据 #{credential.id}？此操作不可撤销。
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="secondary"
              onClick={() => setShowDeleteDialog(false)}
              disabled={deleteCredential.isPending}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={handleDelete}
              disabled={deleteCredential.isPending || !credential.disabled}
            >
              确认删除
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
