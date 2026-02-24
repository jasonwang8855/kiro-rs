import { useMemo, useState } from 'react'
import { LogOut, Plus, RefreshCw, Server, KeyRound, ShieldCheck, Copy } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { storage } from '@/lib/storage'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { CredentialCard } from '@/components/credential-card'
import { BalanceDialog } from '@/components/balance-dialog'
import { AddCredentialDialog } from '@/components/add-credential-dialog'
import { BatchImportDialog } from '@/components/batch-import-dialog'
import { KamImportDialog } from '@/components/kam-import-dialog'
import { KiroOAuthDialog } from '@/components/kiro-oauth-dialog'
import {
  useApiKeys,
  useApiStats,
  useCreateApiKey,
  useCredentials,
  useDeleteApiKey,
  useSetApiKeyDisabled,
} from '@/hooks/use-credentials'
import { extractErrorMessage } from '@/lib/utils'

interface DashboardProps {
  onLogout: () => void
}

export function Dashboard({ onLogout }: DashboardProps) {
  const [selectedCredentialId, setSelectedCredentialId] = useState<number | null>(null)
  const [balanceDialogOpen, setBalanceDialogOpen] = useState(false)
  const [addDialogOpen, setAddDialogOpen] = useState(false)
  const [batchImportDialogOpen, setBatchImportDialogOpen] = useState(false)
  const [kamImportDialogOpen, setKamImportDialogOpen] = useState(false)
  const [oauthDialogOpen, setOauthDialogOpen] = useState(false)
  const [newApiKeyName, setNewApiKeyName] = useState('Default Key')

  const queryClient = useQueryClient()
  const { data, isLoading, error, refetch } = useCredentials()
  const { data: apiKeysData } = useApiKeys()
  const { data: apiStatsData } = useApiStats()
  const { mutate: createApiKey, isPending: creatingApiKey } = useCreateApiKey()
  const { mutate: setApiKeyDisabled } = useSetApiKeyDisabled()
  const { mutate: deleteApiKey } = useDeleteApiKey()

  const sortedApiKeys = useMemo(
    () => [...(apiKeysData?.keys || [])].sort((a, b) => Number(b.enabled) - Number(a.enabled)),
    [apiKeysData?.keys]
  )

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
      toast.error('请输入 API Key 名称')
      return
    }

    createApiKey(
      { name },
      {
        onSuccess: (res) => {
          setNewApiKeyName('')
          toast.success(`创建成功，明文只显示一次：${res.key}`)
        },
        onError: (err) => {
          toast.error(`创建失败: ${extractErrorMessage(err)}`)
        },
      }
    )
  }

  const handleCopy = async (value: string, label = '内容') => {
    try {
      await navigator.clipboard.writeText(value)
      toast.success(`${label}已复制`)
    } catch {
      toast.error(`复制${label}失败`)
    }
  }

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background">
        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background p-4">
        <Card className="w-full max-w-md">
          <CardContent className="pt-6 text-center space-y-4">
            <div className="text-red-500">加载失败：{(error as Error).message}</div>
            <div className="flex justify-center gap-2">
              <Button onClick={() => refetch()}>重试</Button>
              <Button variant="outline" onClick={handleLogout}>重新登录</Button>
            </div>
          </CardContent>
        </Card>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-admin-grid">
      <header className="sticky top-0 z-50 w-full border-b bg-background/95 backdrop-blur">
        <div className="container flex h-14 items-center justify-between px-4 md:px-8">
          <div className="flex items-center gap-2">
            <Server className="h-5 w-5" />
            <span className="font-semibold">Kiro Admin</span>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="icon" onClick={() => refetch()}>
              <RefreshCw className="h-5 w-5" />
            </Button>
            <Button variant="ghost" size="icon" onClick={handleLogout}>
              <LogOut className="h-5 w-5" />
            </Button>
          </div>
        </div>
      </header>

      <main className="container mx-auto px-4 md:px-8 py-6 space-y-6">
        <Card className="overflow-hidden border-0 bg-gradient-to-r from-slate-900 via-sky-900 to-cyan-900 text-white shadow-lg">
          <CardContent className="p-6 md:p-8">
            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div>
                <div className="text-xs uppercase tracking-wider text-sky-200">Control Center</div>
                <div className="text-2xl font-bold mt-1">Kiro Proxy 管理后台</div>
                <div className="text-sm text-sky-100 mt-2">支持 OAuth 自动导入、API Key 管理和调用统计。</div>
              </div>
              <Button
                variant="secondary"
                className="bg-white/95 text-slate-900 hover:bg-white"
                onClick={() => setOauthDialogOpen(true)}
              >
                <ShieldCheck className="h-4 w-4 mr-2" />
                OAuth 一键导入
              </Button>
            </div>
          </CardContent>
        </Card>

        <div className="grid gap-4 md:grid-cols-3">
          <Card className="border-slate-200 bg-gradient-to-br from-white to-slate-50 shadow-sm">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">凭据总数</CardTitle></CardHeader>
            <CardContent><div className="text-2xl font-bold">{data?.total || 0}</div></CardContent>
          </Card>
          <Card className="border-emerald-200 bg-gradient-to-br from-emerald-50 to-lime-50 shadow-sm">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">可用凭据</CardTitle></CardHeader>
            <CardContent><div className="text-2xl font-bold text-green-600">{data?.available || 0}</div></CardContent>
          </Card>
          <Card className="border-sky-200 bg-gradient-to-br from-sky-50 to-blue-50 shadow-sm">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-muted-foreground">当前活跃</CardTitle></CardHeader>
            <CardContent>
              <div className="text-2xl font-bold flex items-center gap-2">#{data?.currentId || '-'} <Badge variant="success">Active</Badge></div>
            </CardContent>
          </Card>
        </div>

        <div className="grid gap-4 md:grid-cols-3">
          <Card className="bg-gradient-to-br from-cyan-50 to-blue-50 border-cyan-100">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-slate-600">API 总调用</CardTitle></CardHeader>
            <CardContent><div className="text-2xl font-bold">{apiStatsData?.overview.totalRequests ?? 0}</div></CardContent>
          </Card>
          <Card className="bg-gradient-to-br from-emerald-50 to-teal-50 border-emerald-100">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-slate-600">输入 Tokens</CardTitle></CardHeader>
            <CardContent><div className="text-2xl font-bold">{apiStatsData?.overview.totalInputTokens ?? 0}</div></CardContent>
          </Card>
          <Card className="bg-gradient-to-br from-amber-50 to-orange-50 border-amber-100">
            <CardHeader className="pb-2"><CardTitle className="text-sm text-slate-600">输出 Tokens</CardTitle></CardHeader>
            <CardContent><div className="text-2xl font-bold">{apiStatsData?.overview.totalOutputTokens ?? 0}</div></CardContent>
          </Card>
        </div>

        <Card className="border-slate-200 bg-white/90 backdrop-blur shadow-sm">
          <CardHeader>
            <CardTitle className="flex items-center gap-2"><KeyRound className="h-4 w-4" />API Key 管理</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex gap-2">
              <Input value={newApiKeyName} onChange={(e) => setNewApiKeyName(e.target.value)} placeholder="新 API Key 名称" />
              <Button onClick={handleCreateApiKey} disabled={creatingApiKey}>创建</Button>
            </div>
            <div className="space-y-2">
              {sortedApiKeys.map((item) => (
                <div
                  key={item.id}
                  className="rounded-xl border border-slate-200 bg-gradient-to-r from-slate-50 to-slate-100/80 p-3 flex items-center justify-between gap-4 shadow-sm"
                >
                  <div className="min-w-0 space-y-1">
                    <div className="font-medium">{item.name}</div>
                    <div className="text-xs font-mono text-slate-700 break-all">{item.key || item.keyPreview}</div>
                    <div className="text-xs text-muted-foreground">
                      req: {item.requestCount} | in: {item.inputTokens} | out: {item.outputTokens}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => handleCopy(item.key || '', 'API Key')}
                      disabled={!item.key}
                    >
                      <Copy className="h-4 w-4 mr-1" />
                      复制
                    </Button>
                    <Switch
                      checked={item.enabled}
                      onCheckedChange={(checked) =>
                        setApiKeyDisabled(
                          { id: item.id, disabled: !checked },
                          { onError: (err) => toast.error(extractErrorMessage(err)) }
                        )
                      }
                    />
                    <Button
                      size="sm"
                      variant="destructive"
                      onClick={() => deleteApiKey(item.id, { onError: (err) => toast.error(extractErrorMessage(err)) })}
                    >
                      删除
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-xl font-semibold">凭据管理</h2>
            <div className="flex gap-2">
              <Button onClick={() => setOauthDialogOpen(true)} size="sm" variant="outline">
                OAuth 导入
              </Button>
              <Button onClick={() => setKamImportDialogOpen(true)} size="sm" variant="outline">KAM 导入</Button>
              <Button onClick={() => setBatchImportDialogOpen(true)} size="sm" variant="outline">批量导入</Button>
              <Button onClick={() => setAddDialogOpen(true)} size="sm"><Plus className="h-4 w-4 mr-2" />添加凭据</Button>
            </div>
          </div>

          {data?.credentials.length === 0 ? (
            <Card>
              <CardContent className="py-8 text-center text-muted-foreground">暂无凭据</CardContent>
            </Card>
          ) : (
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {data?.credentials.map((credential) => (
                <CredentialCard
                  key={credential.id}
                  credential={credential}
                  onViewBalance={handleViewBalance}
                  selected={false}
                  onToggleSelect={() => {}}
                  balance={null}
                  loadingBalance={false}
                />
              ))}
            </div>
          )}
        </div>
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
    </div>
  )
}
