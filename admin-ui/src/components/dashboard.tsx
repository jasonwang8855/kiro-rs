import { useEffect, useMemo, useState } from 'react'
import { LogOut, Plus, RefreshCw, Copy, ShieldCheck, Download, HeartPulse, ChevronDown, ChevronRight } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { storage } from '@/lib/storage'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { CredentialCard } from '@/components/credential-card'
import { BalanceDialog } from '@/components/balance-dialog'
import { AddCredentialDialog } from '@/components/add-credential-dialog'
import { BatchImportDialog } from '@/components/batch-import-dialog'
import { KamImportDialog } from '@/components/kam-import-dialog'
import { KiroOAuthDialog } from '@/components/kiro-oauth-dialog'
import { RequestLogPanel } from '@/components/request-log-panel'
import {
  useApiKeys,
  useApiStats,
  useCreateApiKey,
  useCredentials,
  useDeleteApiKey,
  useSetApiKeyRouting,
  useSetApiKeyDisabled,
  useTotalBalance,
  useLoadBalancingMode,
  useSetLoadBalancingMode,
  useStickyStatus,
  useStickyStreams,
  useStickyStats,
} from '@/hooks/use-credentials'
import { useScrambleText } from '@/hooks/use-scramble-text'
import { extractErrorMessage, copyToClipboard, cn } from '@/lib/utils'
import { exportCredentials, getCredentialBalance } from '@/api/credentials'
import type {
  ApiKeyItem,
  BalanceResponse,
  CredentialSnapshot,
  LoadBalancingMode,
  RoutingMode,
} from '@/types/api'

interface DashboardProps {
  onLogout: () => void
}

const MODE_LABELS: Record<LoadBalancingMode, string> = {
  priority: '优先级',
  balanced: '均衡',
  sticky: '粘性',
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const m = Math.floor(secs / 60)
  const s = secs % 60
  if (m < 60) return `${m}m${s > 0 ? `${s}s` : ''}`
  const h = Math.floor(m / 60)
  return `${h}h${m % 60}m`
}

export function Dashboard({ onLogout }: DashboardProps) {
  const [selectedCredentialId, setSelectedCredentialId] = useState<number | null>(null)
  const [balanceDialogOpen, setBalanceDialogOpen] = useState(false)
  const [addDialogOpen, setAddDialogOpen] = useState(false)
  const [batchImportDialogOpen, setBatchImportDialogOpen] = useState(false)
  const [kamImportDialogOpen, setKamImportDialogOpen] = useState(false)
  const [oauthDialogOpen, setOauthDialogOpen] = useState(false)
  const [newApiKeyName, setNewApiKeyName] = useState('')
  const [newApiKeyRoutingMode, setNewApiKeyRoutingMode] = useState<RoutingMode>('auto')
  const [newApiKeyCredentialId, setNewApiKeyCredentialId] = useState<number | null>(null)
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null)
  const [routingEditKey, setRoutingEditKey] = useState<ApiKeyItem | null>(null)
  const [editRoutingMode, setEditRoutingMode] = useState<RoutingMode>('auto')
  const [editCredentialId, setEditCredentialId] = useState<number | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [batchValidating, setBatchValidating] = useState(false)
  const [streamsExpanded, setStreamsExpanded] = useState(false)

  const queryClient = useQueryClient()
  const { data, isLoading, error, refetch } = useCredentials()
  const { data: apiKeysData } = useApiKeys()
  const { data: apiStatsData } = useApiStats()
  const { data: totalBalanceData } = useTotalBalance()
  const { data: lbModeData, isLoading: lbModeLoading } = useLoadBalancingMode()
  const setLbMode = useSetLoadBalancingMode()
  const currentMode = lbModeData?.mode
  const isSticky = currentMode === 'sticky'
  const { data: stickyStatusData, isError: stickyStatusError } = useStickyStatus(isSticky)
  const { data: stickyStreamsData } = useStickyStreams(isSticky && streamsExpanded)
  const { data: stickyStatsData, isError: stickyStatsError } = useStickyStats(isSticky)
  const stickyFetchError = stickyStatusError || stickyStatsError
  const { mutate: createApiKey, isPending: creatingApiKey } = useCreateApiKey()
  const { mutate: setApiKeyRouting, isPending: settingApiKeyRouting } = useSetApiKeyRouting()
  const { mutate: setApiKeyDisabled } = useSetApiKeyDisabled()
  const { mutate: deleteApiKey } = useDeleteApiKey()
  const totalCredentialsDisplay = useScrambleText(String(data?.total || 0), !isLoading)
  const activeCredentialsDisplay = useScrambleText(String(data?.available || 0), !isLoading)
  const apiRequestsDisplay = useScrambleText(String(apiStatsData?.overview.totalRequests ?? 0), !isLoading)

  const [balances, setBalances] = useState<Record<number, BalanceResponse>>({})
  const [loadingBalances, setLoadingBalances] = useState<Record<number, boolean>>({})

  useEffect(() => {
    if (!data?.credentials?.length) return
    for (const cred of data.credentials) {
      if (balances[cred.id] || loadingBalances[cred.id]) continue
      setLoadingBalances((prev) => ({ ...prev, [cred.id]: true }))
      getCredentialBalance(cred.id)
        .then((b) => setBalances((prev) => ({ ...prev, [cred.id]: b })))
        .catch(() => {})
        .finally(() => setLoadingBalances((prev) => ({ ...prev, [cred.id]: false })))
    }
  }, [data?.credentials])

  const sortedApiKeys = useMemo(
    () => [...(apiKeysData?.keys || [])].sort((a, b) => Number(b.enabled) - Number(a.enabled)),
    [apiKeysData?.keys]
  )
  const availableCredentials = useMemo(() => data?.credentials || [], [data?.credentials])

  const handleLogout = () => {
    storage.removeToken()
    queryClient.clear()
    onLogout()
  }

  const handleViewBalance = (id: number) => {
    setSelectedCredentialId(id)
    setBalanceDialogOpen(true)
  }

  const handleCreateApiKey = () => {
    const name = newApiKeyName.trim()
    if (!name) {
      toast.error('请输入 API 密钥名称')
      return
    }
    if (newApiKeyRoutingMode === 'fixed' && newApiKeyCredentialId === null) {
      toast.error('固定路由模式必须选择绑定凭据')
      return
    }

    createApiKey(
      {
        name,
        routingMode: newApiKeyRoutingMode,
        credentialId: newApiKeyRoutingMode === 'fixed' ? newApiKeyCredentialId ?? undefined : undefined,
      },
      {
        onSuccess: (res) => {
          setNewApiKeyName('')
          setNewApiKeyRoutingMode('auto')
          setNewApiKeyCredentialId(null)
          toast.success(`创建成功，明文只显示一次：${res.key}`)
        },
        onError: (err) => {
          toast.error(`创建失败: ${extractErrorMessage(err)}`)
        },
      }
    )
  }

  const openRoutingDialog = (item: ApiKeyItem) => {
    setRoutingEditKey(item)
    setEditRoutingMode(item.routingMode)
    setEditCredentialId(item.credentialId)
  }

  const handleUpdateApiKeyRouting = () => {
    if (!routingEditKey) return
    if (editRoutingMode === 'fixed' && editCredentialId === null) {
      toast.error('固定路由模式必须选择绑定凭据')
      return
    }

    setApiKeyRouting(
      {
        id: routingEditKey.id,
        routingMode: editRoutingMode,
        credentialId: editRoutingMode === 'fixed' ? editCredentialId ?? undefined : undefined,
      },
      {
        onSuccess: () => {
          toast.success('路由设置已更新')
          setRoutingEditKey(null)
        },
        onError: (err) => {
          toast.error(`更新失败: ${extractErrorMessage(err)}`)
        },
      }
    )
  }

  const handleCopy = async (value: string, label = '内容') => {
    try {
      await copyToClipboard(value)
      toast.success(`${label}已复制`)
    } catch {
      toast.error(`复制${label}失败`)
    }
  }

  const handleExport = async () => {
    try {
      const credentials = await exportCredentials()
      const json = JSON.stringify(credentials, null, 2)
      const blob = new Blob([json], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `credentials-export-${Date.now()}.json`
      a.click()
      URL.revokeObjectURL(url)
      toast.success('凭据导出成功')
    } catch (err) {
      toast.error(`导出失败: ${extractErrorMessage(err)}`)
    }
  }

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const toggleSelectAll = () => {
    if (!data?.credentials?.length) return
    if (selectedIds.size === data.credentials.length) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(data.credentials.map((c) => c.id)))
    }
  }

  const handleBatchValidate = async () => {
    if (selectedIds.size === 0) return
    setBatchValidating(true)
    let ok = 0
    let fail = 0
    for (const id of selectedIds) {
      try {
        const b = await getCredentialBalance(id)
        setBalances((prev) => ({ ...prev, [id]: b }))
        ok++
      } catch {
        fail++
      }
    }
    setBatchValidating(false)
    toast.success(`验活完成：${ok} 成功${fail > 0 ? `，${fail} 失败` : ''}`)
  }

  const handleModeChange = (mode: LoadBalancingMode) => {
    if (mode === currentMode) return
    setLbMode.mutate(mode, {
      onSuccess: () => toast.success(`负载均衡模式已切换为「${MODE_LABELS[mode]}」`),
      onError: (err) => toast.error(`切换失败: ${extractErrorMessage(err)}`),
    })
  }

  const stickySnapshotMap = useMemo(() => {
    const map = new Map<number, CredentialSnapshot>()
    if (stickyStatusData?.credentials) {
      for (const c of stickyStatusData.credentials) {
        map.set(c.id, c)
      }
    }
    return map
  }, [stickyStatusData])



  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-black">
        <div className="orbital-loader" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-black p-4">
        <Card className="w-full max-w-md">
          <CardContent className="space-y-4 pt-6 text-center">
            <div className="text-red-400">加载失败：{(error as Error).message}</div>
            <div className="flex justify-center gap-2">
              <Button onClick={() => refetch()}>重试</Button>
              <Button variant="secondary" onClick={handleLogout}>
                重新登录
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-black">
      <main
        className="mx-auto grid max-w-[1600px] grid-cols-1 gap-4 p-6 md:grid-cols-12"
      >
        <section className="col-span-1 flex flex-col gap-3 md:col-span-12 md:flex-row md:items-center md:justify-between">
          <div className="font-mono text-xs tracking-normal text-neutral-500">
            KIRO-RS // 控制中心
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <div className="inline-flex items-center rounded-md border border-white/10 bg-white/[0.02] p-0.5">
              {(['priority', 'balanced', 'sticky'] as const).map((mode) => (
                <button
                  key={mode}
                  type="button"
                  onClick={() => handleModeChange(mode)}
                  disabled={setLbMode.isPending || lbModeLoading}
                  className={cn(
                    'rounded-[5px] px-2.5 py-1 font-sans text-xs font-medium transition-colors',
                    currentMode === mode
                      ? 'bg-white/10 text-white'
                      : 'text-neutral-500 hover:text-neutral-300',
                    lbModeLoading && 'opacity-50'
                  )}
                >
                  {MODE_LABELS[mode]}
                </button>
              ))}
            </div>
            <div className="h-5 w-px bg-white/10" />
            <Button onClick={() => setOauthDialogOpen(true)} size="sm" variant="secondary">
              <ShieldCheck className="mr-2 h-4 w-4" />
              OAuth 导入
            </Button>
            <Button onClick={() => setKamImportDialogOpen(true)} size="sm" variant="secondary">
              KAM 导入
            </Button>
            <Button onClick={() => setBatchImportDialogOpen(true)} size="sm" variant="secondary">
              批量导入
            </Button>
            <Button onClick={() => setAddDialogOpen(true)} size="sm">
              <Plus className="mr-2 h-4 w-4" />
              添加凭据
            </Button>
            <Button onClick={handleExport} size="sm" variant="secondary">
              <Download className="mr-2 h-4 w-4" />
              导出
            </Button>
            <Button variant="secondary" size="icon" onClick={() => refetch()}>
              <RefreshCw className="h-4 w-4" />
            </Button>
            <Button variant="secondary" size="icon" onClick={handleLogout}>
              <LogOut className="h-4 w-4" />
            </Button>
          </div>
        </section>

        <Card className="col-span-1 md:col-span-3 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">总凭据数</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-5xl font-mono font-light tracking-tight text-white">{totalCredentialsDisplay}</div>
          </CardContent>
        </Card>

        <Card className="col-span-1 md:col-span-3 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">活跃凭据</CardTitle>
          </CardHeader>
          <CardContent className="flex items-end justify-between gap-3">
            <div className="text-5xl font-mono font-light tracking-tight text-white">{activeCredentialsDisplay}</div>
            <Badge variant="secondary" className="mb-1 font-mono text-[10px] tracking-wider text-neutral-400">当前 #{data?.currentId || '-'}</Badge>
          </CardContent>
        </Card>

        <Card className="col-span-1 md:col-span-3 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">API 请求量</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="text-5xl font-mono font-light tracking-tight text-white">{apiRequestsDisplay}</div>
            <div className="text-xs font-mono tracking-widest text-neutral-500 uppercase">
              IN <span className="text-white">{apiStatsData?.overview.totalInputTokens ?? 0}</span> <span className="text-neutral-700">/</span> OUT <span className="text-white">{apiStatsData?.overview.totalOutputTokens ?? 0}</span>
            </div>
          </CardContent>
        </Card>

        <Card className="col-span-1 md:col-span-3 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">总额度</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="text-5xl font-mono font-light tracking-tight text-white">
              {totalBalanceData ? totalBalanceData.totalRemaining.toFixed(1) : '-'}
            </div>
            <div className="text-xs font-mono tracking-widest text-neutral-500 uppercase">
              已用 <span className="text-white">{totalBalanceData?.totalCurrentUsage.toFixed(1) ?? '-'}</span> <span className="text-neutral-700">/</span> 总计 <span className="text-white">{totalBalanceData?.totalUsageLimit.toFixed(1) ?? '-'}</span>
            </div>
          </CardContent>
        </Card>

        {isSticky ? (
          <section className="col-span-1 md:col-span-12 mt-2">
            <div className="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-lg border border-white/10 bg-[#050505] px-4 py-2.5">
              <span className="text-[11px] font-sans font-medium tracking-wide text-neutral-500">路由指标</span>
              {stickyFetchError && (
                <span className="text-[11px] font-sans text-yellow-500/80">数据可能过期</span>
              )}
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">活跃流</span>
                <span className="font-mono text-sm tabular-nums text-white">{stickyStatusData?.activeStreamCount ?? 0}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">命中</span>
                <span className="font-mono text-sm tabular-nums text-white">{stickyStatsData?.stats.hits ?? 0}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">分配</span>
                <span className="font-mono text-sm tabular-nums text-white">{stickyStatsData?.stats.assignments ?? 0}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">解绑</span>
                <span className="font-mono text-sm tabular-nums text-white">{stickyStatsData?.stats.unbinds ?? 0}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">插队</span>
                <span className="font-mono text-sm tabular-nums text-white">{stickyStatsData?.stats.queueJumps ?? 0}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className="text-[11px] font-sans text-neutral-500">429</span>
                <span className={cn(
                  'font-mono text-sm tabular-nums',
                  (stickyStatsData?.stats.rejections429 ?? 0) > 0 ? 'text-red-400' : 'text-white'
                )}>
                  {stickyStatsData?.stats.rejections429 ?? 0}
                </span>
              </div>
              <div className="ml-auto">
                <button
                  type="button"
                  className="flex items-center gap-1 text-[11px] font-sans text-neutral-500 hover:text-neutral-300 transition-colors"
                  onClick={() => setStreamsExpanded(!streamsExpanded)}
                >
                  {streamsExpanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
                  活跃流详情
                </button>
              </div>
            </div>
            {streamsExpanded && (
              <div className="mt-2 overflow-x-auto rounded-lg border border-white/10 bg-[#050505]">
                <table className="w-full min-w-[700px] border-collapse">
                  <thead>
                    <tr className="border-b border-white/10">
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">Stream ID</th>
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">凭据 ID</th>
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">API Key</th>
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">Session</th>
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">空闲时间</th>
                      <th className="px-3 py-2 text-left font-sans text-[11px] font-medium tracking-wide text-neutral-500">状态</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(!stickyStreamsData?.streams || stickyStreamsData.streams.length === 0) ? (
                      <tr>
                        <td colSpan={6} className="px-3 py-4 text-center font-sans text-xs text-neutral-500">暂无活跃流</td>
                      </tr>
                    ) : (
                      stickyStreamsData.streams.map((s) => (
                        <tr key={s.streamId} className="border-b border-white/5 font-mono text-xs text-white">
                          <td className="px-3 py-2 tabular-nums text-neutral-400">{s.streamId}</td>
                          <td className="px-3 py-2 tabular-nums">#{s.credentialId}</td>
                          <td className="px-3 py-2 text-neutral-400">{s.apiKey.length > 16 ? s.apiKey.slice(0, 8) + '...' + s.apiKey.slice(-4) : s.apiKey}</td>
                          <td className="px-3 py-2 text-neutral-400">{s.sessionId ? (s.sessionId.length > 16 ? s.sessionId.slice(0, 8) + '...' : s.sessionId) : '-'}</td>
                          <td className="px-3 py-2 tabular-nums">{formatDuration(s.startedAtSecsAgo)}</td>
                          <td className="px-3 py-2">
                            <span className={cn(
                              'inline-flex items-center gap-1.5 text-[11px]',
                              s.activated ? 'text-emerald-400' : 'text-yellow-400'
                            )}>
                              <div className={cn('h-1.5 w-1.5 rounded-full', s.activated ? 'bg-emerald-400' : 'bg-yellow-400')} />
                              {s.activated ? '活跃' : '预留'}
                            </span>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        ) : (
          <section className="col-span-1 md:col-span-12 mt-2">
            <div className="flex items-center gap-2 rounded-lg border border-white/5 bg-[#050505]/50 px-4 py-2">
              <span className="text-[11px] font-sans text-neutral-600">
                当前路由模式：{currentMode ? MODE_LABELS[currentMode] : '加载中...'}
              </span>
            </div>
          </section>
        )}

        <section className="col-span-1 md:col-span-12 mt-4">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2 px-1">
            <h2 className="font-sans text-sm font-medium tracking-wide text-neutral-500">凭据列表</h2>
            {data?.credentials && data.credentials.length > 0 && (
              <div className="flex flex-wrap items-center gap-2">
                <Button size="sm" variant="secondary" onClick={toggleSelectAll}>
                  {selectedIds.size === data.credentials.length ? '取消全选' : '全选'}
                </Button>
                {selectedIds.size > 0 && (
                  <>
                    <span className="text-xs font-mono text-neutral-500">已选 {selectedIds.size}</span>
                    <Button size="sm" variant="secondary" onClick={handleBatchValidate} disabled={batchValidating}>
                      <HeartPulse className="mr-1 h-4 w-4" />
                      {batchValidating ? '验活中...' : '批量验活'}
                    </Button>
                  </>
                )}
              </div>
            )}
          </div>
          <div>
            {data?.credentials.length === 0 ? (
              <div className="ghost-credentials relative grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                <div className="h-40 rounded-lg border border-white/5 bg-black/20" />
                <div className="h-40 rounded-lg border border-white/5 bg-black/20" />
                <div className="h-40 rounded-lg border border-white/5 bg-black/20" />
                <div className="h-40 rounded-lg border border-white/5 bg-black/20" />
                <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
                  <span className="font-sans text-sm font-medium text-neutral-500">暂无凭据配置</span>
                </div>
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                {data?.credentials.map((credential) => (
                  <CredentialCard
                    key={credential.id}
                    credential={credential}
                    onViewBalance={handleViewBalance}
                    selected={selectedIds.has(credential.id)}
                    onToggleSelect={() => toggleSelect(credential.id)}
                    balance={balances[credential.id] ?? null}
                    loadingBalance={loadingBalances[credential.id] ?? false}
                    stickyMode={isSticky}
                    stickySnapshot={stickySnapshotMap.get(credential.id)}
                  />
                ))}
              </div>
            )}
          </div>
        </section>

        <section className="col-span-1 md:col-span-12 mt-4">
          <h2 className="mb-4 px-1 font-mono text-sm tracking-normal text-neutral-400">API 密钥</h2>
          <div className="space-y-4">
            <div className="flex flex-col gap-2 sm:flex-row">
              <Input
                value={newApiKeyName}
                onChange={(e) => setNewApiKeyName(e.target.value)}
                placeholder="新 API 密钥名称"
                className="font-mono max-w-md"
              />
              <Select
                value={newApiKeyRoutingMode}
                onChange={(e) => {
                  const mode = e.target.value as RoutingMode
                  setNewApiKeyRoutingMode(mode)
                  if (mode === 'auto') {
                    setNewApiKeyCredentialId(null)
                  }
                }}
                className="w-full sm:w-40"
              >
                <option value="auto">自动</option>
                <option value="fixed">固定</option>
              </Select>
              {newApiKeyRoutingMode === 'fixed' && (
                <Select
                  value={newApiKeyCredentialId !== null ? String(newApiKeyCredentialId) : ''}
                  onChange={(e) => setNewApiKeyCredentialId(e.target.value ? Number(e.target.value) : null)}
                  className="w-full sm:w-44"
                >
                  <option value="">选择凭据</option>
                  {availableCredentials.map((cred) => (
                    <option key={cred.id} value={cred.id}>
                      #{cred.id} (P{cred.priority})
                    </option>
                  ))}
                </Select>
              )}
              <Button onClick={handleCreateApiKey} disabled={creatingApiKey} className="sm:w-auto">
                创建
              </Button>
            </div>

            <div className="overflow-x-auto rounded-lg border border-white/10 bg-[#050505]">
              <table className="w-full min-w-[980px] border-collapse">
                <thead>
                  <tr className="border-b border-white/10">
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">名称</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">密钥</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">路由</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">统计</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">状态</th>
                    <th className="px-3 py-2 text-right font-sans text-xs font-medium tracking-wide text-neutral-500">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {sortedApiKeys.length === 0 && (
                    <tr>
                      <td colSpan={6} className="px-3 py-8 text-center font-sans text-sm font-medium text-neutral-500">
                        暂无 API 密钥
                      </td>
                    </tr>
                  )}
                  {sortedApiKeys.map((item) => (
                    <tr key={item.id} className="border-b border-white/5 font-mono text-sm text-white">
                      <td className="px-3 py-3 font-sans font-medium text-neutral-200">{item.name}</td>
                      <td className="max-w-[420px] break-all px-3 py-3 text-neutral-400">{item.key || item.keyPreview}</td>
                      <td className="px-3 py-3">
                        {item.routingMode === 'fixed' ? (
                          <div className="flex items-center gap-2">
                            <Badge className="border-sky-500/30 bg-sky-500/15 text-sky-300">固定</Badge>
                            <span className="font-sans text-xs text-sky-200/90">
                              #{item.credentialId ?? '-'}
                            </span>
                          </div>
                        ) : (
                          <Badge variant="secondary">自动</Badge>
                        )}
                      </td>
                      <td className="px-3 py-3 text-neutral-400 font-sans text-xs">
                        请求 <span className="font-mono text-white text-sm">{item.requestCount}</span> <span className="text-neutral-700">|</span> 输入 <span className="font-mono text-white text-sm">{item.inputTokens}</span> <span className="text-neutral-700">|</span> 输出 <span className="font-mono text-white text-sm">{item.outputTokens}</span>
                      </td>
                      <td className="px-3 py-3">
                        <Switch
                          checked={item.enabled}
                          onCheckedChange={(checked) =>
                            setApiKeyDisabled(
                              { id: item.id, disabled: !checked },
                              { onError: (err) => toast.error(extractErrorMessage(err)) }
                            )
                          }
                        />
                      </td>
                      <td className="px-3 py-3">
                        <div className="flex justify-end gap-2 flex-wrap">
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() => openRoutingDialog(item)}
                          >
                            路由设置
                          </Button>
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() => handleCopy(item.key || '', 'API 密钥')}
                            disabled={!item.key}
                          >
                            <Copy className="mr-1 h-4 w-4" />
                            复制
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            onClick={() => setDeleteKeyId(item.id)}
                          >
                            删除
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </section>

        <RequestLogPanel />
      </main>

      <BalanceDialog
        credentialId={selectedCredentialId}
        open={balanceDialogOpen}
        onOpenChange={setBalanceDialogOpen}
      />

      <AddCredentialDialog open={addDialogOpen} onOpenChange={setAddDialogOpen} />
      <BatchImportDialog open={batchImportDialogOpen} onOpenChange={setBatchImportDialogOpen} />
      <KamImportDialog open={kamImportDialogOpen} onOpenChange={setKamImportDialogOpen} />
      <KiroOAuthDialog
        open={oauthDialogOpen}
        onOpenChange={setOauthDialogOpen}
        baselineTotal={data?.total || 0}
        onImported={() => {
          refetch()
          queryClient.invalidateQueries({ queryKey: ['credentials'] })
        }}
      />

      <Dialog open={routingEditKey !== null} onOpenChange={(open) => !open && setRoutingEditKey(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>路由设置</DialogTitle>
            <DialogDescription>
              {routingEditKey ? `为「${routingEditKey.name}」设置路由模式` : '设置路由模式'}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1.5">
              <div className="text-xs font-sans text-neutral-500">路由模式</div>
              <Select
                value={editRoutingMode}
                onChange={(e) => {
                  const mode = e.target.value as RoutingMode
                  setEditRoutingMode(mode)
                  if (mode === 'auto') {
                    setEditCredentialId(null)
                  }
                }}
              >
                <option value="auto">自动</option>
                <option value="fixed">固定</option>
              </Select>
            </div>

            {editRoutingMode === 'fixed' && (
              <div className="space-y-1.5">
                <div className="text-xs font-sans text-neutral-500">绑定凭据</div>
                <Select
                  value={editCredentialId !== null ? String(editCredentialId) : ''}
                  onChange={(e) => setEditCredentialId(e.target.value ? Number(e.target.value) : null)}
                >
                  <option value="">选择凭据</option>
                  {availableCredentials.map((cred) => (
                    <option key={cred.id} value={cred.id}>
                      #{cred.id} (P{cred.priority}) {cred.email ? `- ${cred.email}` : ''}
                    </option>
                  ))}
                </Select>
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setRoutingEditKey(null)}>
              取消
            </Button>
            <Button onClick={handleUpdateApiKeyRouting} disabled={settingApiKeyRouting}>
              保存
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteKeyId !== null} onOpenChange={(open) => !open && setDeleteKeyId(null)}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>删除 API 密钥</DialogTitle>
            <DialogDescription>此操作不可撤销，确认删除？</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setDeleteKeyId(null)}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (!deleteKeyId) return
                deleteApiKey(deleteKeyId, { onError: (err) => toast.error(extractErrorMessage(err)) })
                setDeleteKeyId(null)
              }}
            >
              确认
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
