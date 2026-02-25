import { useMemo, useState } from 'react'
import { LogOut, Plus, RefreshCw, Copy, ShieldCheck } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { storage } from '@/lib/storage'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
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
import {
  useApiKeys,
  useApiStats,
  useCreateApiKey,
  useCredentials,
  useDeleteApiKey,
  useSetApiKeyDisabled,
} from '@/hooks/use-credentials'
import { useScrambleText } from '@/hooks/use-scramble-text'
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
  const [newApiKeyName, setNewApiKeyName] = useState('')
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null)

  const queryClient = useQueryClient()
  const { data, isLoading, error, refetch } = useCredentials()
  const { data: apiKeysData } = useApiKeys()
  const { data: apiStatsData } = useApiStats()
  const { mutate: createApiKey, isPending: creatingApiKey } = useCreateApiKey()
  const { mutate: setApiKeyDisabled } = useSetApiKeyDisabled()
  const { mutate: deleteApiKey } = useDeleteApiKey()
  const totalCredentialsDisplay = useScrambleText(String(data?.total || 0), !isLoading)
  const activeCredentialsDisplay = useScrambleText(String(data?.available || 0), !isLoading)
  const apiRequestsDisplay = useScrambleText(String(apiStatsData?.overview.totalRequests ?? 0), !isLoading)

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
      toast.error('请输入 API 密钥名称')
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
            <Button variant="secondary" size="icon" onClick={() => refetch()}>
              <RefreshCw className="h-4 w-4" />
            </Button>
            <Button variant="secondary" size="icon" onClick={handleLogout}>
              <LogOut className="h-4 w-4" />
            </Button>
          </div>
        </section>

        <Card className="col-span-1 md:col-span-4 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">总凭据数</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-5xl font-mono font-light tracking-tight text-white">{totalCredentialsDisplay}</div>
          </CardContent>
        </Card>

        <Card className="col-span-1 md:col-span-4 border-white/10 bg-[#050505]">
          <CardHeader className="pb-3">
            <CardTitle className="text-xs font-sans font-medium tracking-wide text-neutral-500">活跃凭据</CardTitle>
          </CardHeader>
          <CardContent className="flex items-end justify-between gap-3">
            <div className="text-5xl font-mono font-light tracking-tight text-white">{activeCredentialsDisplay}</div>
            <Badge variant="secondary" className="mb-1 font-mono text-[10px] tracking-wider text-neutral-400">当前 #{data?.currentId || '-'}</Badge>
          </CardContent>
        </Card>

        <Card className="col-span-1 md:col-span-4 border-white/10 bg-[#050505]">
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

        <section className="col-span-1 md:col-span-12 mt-4">
          <h2 className="mb-4 px-1 font-sans text-sm font-medium tracking-wide text-neutral-500">凭据列表</h2>
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
                    selected={false}
                    onToggleSelect={() => {}}
                    balance={null}
                    loadingBalance={false}
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
              <Button onClick={handleCreateApiKey} disabled={creatingApiKey} className="sm:w-auto">
                创建
              </Button>
            </div>

            <div className="overflow-x-auto rounded-lg border border-white/10 bg-[#050505]">
              <table className="w-full min-w-[860px] border-collapse">
                <thead>
                  <tr className="border-b border-white/10">
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">名称</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">密钥</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">统计</th>
                    <th className="px-3 py-2 text-left font-sans text-xs font-medium tracking-wide text-neutral-500">状态</th>
                    <th className="px-3 py-2 text-right font-sans text-xs font-medium tracking-wide text-neutral-500">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {sortedApiKeys.length === 0 && (
                    <tr>
                      <td colSpan={5} className="px-3 py-8 text-center font-sans text-sm font-medium text-neutral-500">
                        暂无 API 密钥
                      </td>
                    </tr>
                  )}
                  {sortedApiKeys.map((item) => (
                    <tr key={item.id} className="border-b border-white/5 font-mono text-sm text-white">
                      <td className="px-3 py-3 font-sans font-medium text-neutral-200">{item.name}</td>
                      <td className="max-w-[420px] break-all px-3 py-3 text-neutral-400">{item.key || item.keyPreview}</td>
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
                        <div className="flex justify-end gap-2">
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
