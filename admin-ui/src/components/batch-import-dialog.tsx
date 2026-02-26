import { useState } from 'react'
import { toast } from 'sonner'
import { CheckCircle2, XCircle, AlertCircle } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useCredentials, useAddCredential, useDeleteCredential } from '@/hooks/use-credentials'
import { getCredentialBalance, setCredentialDisabled } from '@/api/credentials'
import { extractErrorMessage } from '@/lib/utils'

interface BatchImportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

interface CredentialInput {
  refreshToken: string
  clientId?: string
  clientSecret?: string
  region?: string
  authRegion?: string
  apiRegion?: string
  priority?: number
  machineId?: string
}

interface VerificationResult {
  index: number
  status: 'pending' | 'checking' | 'verifying' | 'verified' | 'duplicate' | 'failed'
  error?: string
  usage?: string
  email?: string
  credentialId?: number
  rollbackStatus?: 'success' | 'failed' | 'skipped'
  rollbackError?: string
}

async function sha256Hex(value: string): Promise<string> {
  const encoded = new TextEncoder().encode(value)
  const digest = await crypto.subtle.digest('SHA-256', encoded)
  const bytes = new Uint8Array(digest)
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export function BatchImportDialog({ open, onOpenChange }: BatchImportDialogProps) {
  const [jsonInput, setJsonInput] = useState('')
  const [importing, setImporting] = useState(false)
  const [progress, setProgress] = useState({ current: 0, total: 0 })
  const [currentProcessing, setCurrentProcessing] = useState<string>('')
  const [results, setResults] = useState<VerificationResult[]>([])

  const { data: existingCredentials } = useCredentials()
  const { mutateAsync: addCredential } = useAddCredential()
  const { mutateAsync: deleteCredential } = useDeleteCredential()

  const rollbackCredential = async (id: number): Promise<{ success: boolean; error?: string }> => {
    try {
      await setCredentialDisabled(id, true)
    } catch (error) {
      return {
        success: false,
        error: `禁用失败: ${extractErrorMessage(error)}`,
      }
    }

    try {
      await deleteCredential(id)
      return { success: true }
    } catch (error) {
      return {
        success: false,
        error: `删除失败: ${extractErrorMessage(error)}`,
      }
    }
  }

  const resetForm = () => {
    setJsonInput('')
    setProgress({ current: 0, total: 0 })
    setCurrentProcessing('')
    setResults([])
  }

  const handleBatchImport = async () => {
    try {
      const parsed = JSON.parse(jsonInput)
      const credentials: CredentialInput[] = Array.isArray(parsed) ? parsed : [parsed]

      if (credentials.length === 0) {
        toast.error('无可导入凭据')
        return
      }

      setImporting(true)
      setProgress({ current: 0, total: credentials.length })

      const initialResults: VerificationResult[] = credentials.map((_, i) => ({
        index: i + 1,
        status: 'pending',
      }))
      setResults(initialResults)

      const existingTokenHashes = new Set(
        existingCredentials?.credentials
          .map((c) => c.refreshTokenHash)
          .filter((hash): hash is string => Boolean(hash)) || []
      )

      let successCount = 0
      let duplicateCount = 0
      let failCount = 0
      let rollbackSuccessCount = 0
      let rollbackFailedCount = 0
      let rollbackSkippedCount = 0

      for (let i = 0; i < credentials.length; i++) {
        const cred = credentials[i]
        const token = cred.refreshToken?.trim()

        setCurrentProcessing(`正在处理凭据 ${i + 1}/${credentials.length}`)
        setResults((prev) => {
          const next = [...prev]
          next[i] = { ...next[i], status: 'checking' }
          return next
        })

        if (!token) {
          failCount++
          setResults((prev) => {
            const next = [...prev]
            next[i] = { ...next[i], status: 'failed', error: '缺少 refreshToken', rollbackStatus: 'skipped' }
            return next
          })
          setProgress({ current: i + 1, total: credentials.length })
          continue
        }

        const tokenHash = await sha256Hex(token)

        if (existingTokenHashes.has(tokenHash)) {
          duplicateCount++
          const existingCred = existingCredentials?.credentials.find((c) => c.refreshTokenHash === tokenHash)
          setResults((prev) => {
            const next = [...prev]
            next[i] = {
              ...next[i],
              status: 'duplicate',
              error: '凭据已存在',
              email: existingCred?.email || undefined,
            }
            return next
          })
          setProgress({ current: i + 1, total: credentials.length })
          continue
        }

        setResults((prev) => {
          const next = [...prev]
          next[i] = { ...next[i], status: 'verifying' }
          return next
        })

        let addedCredId: number | null = null

        try {
          const clientId = cred.clientId?.trim() || undefined
          const clientSecret = cred.clientSecret?.trim() || undefined

          if ((clientId && !clientSecret) || (!clientId && clientSecret)) {
            throw new Error('idc 模式必须同时提供 clientId 和 clientSecret')
          }

          const authMethod = clientId && clientSecret ? 'idc' : 'social'

          const addedCred = await addCredential({
            refreshToken: token,
            authMethod,
            authRegion: cred.authRegion?.trim() || cred.region?.trim() || undefined,
            apiRegion: cred.apiRegion?.trim() || undefined,
            clientId,
            clientSecret,
            priority: cred.priority || 0,
            machineId: cred.machineId?.trim() || undefined,
          })

          addedCredId = addedCred.credentialId
          await new Promise((resolve) => setTimeout(resolve, 1000))

          const balance = await getCredentialBalance(addedCred.credentialId)

          successCount++
          existingTokenHashes.add(tokenHash)
          setCurrentProcessing(addedCred.email ? `已验证：${addedCred.email}` : `已验证凭据 ${i + 1}`)
          setResults((prev) => {
            const next = [...prev]
            next[i] = {
              ...next[i],
              status: 'verified',
              usage: `${balance.currentUsage}/${balance.usageLimit}`,
              email: addedCred.email || undefined,
              credentialId: addedCred.credentialId,
            }
            return next
          })
        } catch (error) {
          let rollbackStatus: VerificationResult['rollbackStatus'] = 'skipped'
          let rollbackError: string | undefined

          if (addedCredId) {
            const rollbackResult = await rollbackCredential(addedCredId)
            if (rollbackResult.success) {
              rollbackStatus = 'success'
              rollbackSuccessCount++
            } else {
              rollbackStatus = 'failed'
              rollbackFailedCount++
              rollbackError = rollbackResult.error
            }
          } else {
            rollbackSkippedCount++
          }

          failCount++
          setResults((prev) => {
            const next = [...prev]
            next[i] = {
              ...next[i],
              status: 'failed',
              error: extractErrorMessage(error),
              email: undefined,
              rollbackStatus,
              rollbackError,
            }
            return next
          })
        }

        setProgress({ current: i + 1, total: credentials.length })
      }

      if (failCount === 0 && duplicateCount === 0) {
        toast.success(`已导入并验证 ${successCount} 个凭据`)
      } else {
        const failureSummary =
          failCount > 0
            ? `，失败 ${failCount}（回滚成功 ${rollbackSuccessCount}，回滚失败 ${rollbackFailedCount}，跳过回滚 ${rollbackSkippedCount}）`
            : ''
        toast.info(`验证完成：成功 ${successCount}，重复 ${duplicateCount}${failureSummary}`)

        if (rollbackFailedCount > 0) {
          toast.warning(`有 ${rollbackFailedCount} 个凭据回滚失败，请手动禁用或删除。`)
        }
      }
    } catch (error) {
      toast.error('JSON 解析失败: ' + extractErrorMessage(error))
    } finally {
      setImporting(false)
    }
  }

  const getStatusIcon = (status: VerificationResult['status']) => {
    switch (status) {
      case 'pending':
        return <div className="h-5 w-5 rounded-full border-2 border-gray-300" />
      case 'checking':
      case 'verifying':
        return <div className="orbital-loader scale-90" />
      case 'verified':
        return <CheckCircle2 className="h-5 w-5 text-green-500" />
      case 'duplicate':
        return <AlertCircle className="h-5 w-5 text-yellow-500" />
      case 'failed':
        return <XCircle className="h-5 w-5 text-red-500" />
    }
  }

  const getStatusText = (result: VerificationResult) => {
    switch (result.status) {
      case 'pending':
        return '待处理'
      case 'checking':
        return '检查重复中...'
      case 'verifying':
        return '验证中...'
      case 'verified':
        return '已验证'
      case 'duplicate':
        return '重复'
      case 'failed':
        if (result.rollbackStatus === 'success') return '失败（已回滚）'
        if (result.rollbackStatus === 'failed') return '失败（回滚失败）'
        return '失败'
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(newOpen) => {
        if (!newOpen && !importing) {
          resetForm()
        }
        onOpenChange(newOpen)
      }}
    >
      <DialogContent className="flex max-h-[80vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="font-mono text-sm tracking-normal text-neutral-400">批量导入（自动验证）</DialogTitle>
        </DialogHeader>

        <div className="flex-1 space-y-4 overflow-y-auto py-4">
          <div className="space-y-2">
            <label className="font-mono text-xs tracking-normal text-neutral-400">凭据 JSON</label>
            <textarea
              placeholder={'粘贴凭据 JSON 数组或对象\n示例: [{"refreshToken":"...","clientId":"...","clientSecret":"...","authRegion":"us-east-1","apiRegion":"us-west-2"}]'}
              value={jsonInput}
              onChange={(e) => setJsonInput(e.target.value)}
              disabled={importing}
              className="flex min-h-[200px] w-full rounded-md border border-white/10 bg-[#030303] p-4 font-mono text-xs text-white ring-offset-background placeholder:text-neutral-600 focus-visible:border-white/30 focus-visible:outline-none focus-visible:ring-0 disabled:cursor-not-allowed disabled:opacity-50"
            />
            <p className="text-xs text-neutral-400">验证失败会在可行时自动回滚。</p>
          </div>

          {(importing || results.length > 0) && (
            <>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span>{importing ? '验证进度' : '验证完成'}</span>
                  <span>
                    {progress.current} / {progress.total}
                  </span>
                </div>
                <div className="h-2 w-full rounded-full bg-white/10">
                  <div
                    className="h-2 rounded-full bg-gradient-to-r from-neutral-500 to-white transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]"
                    style={{ width: `${progress.total > 0 ? (progress.current / progress.total) * 100 : 0}%` }}
                  />
                </div>
                {importing && currentProcessing && <div className="text-xs text-neutral-400">{currentProcessing}</div>}
              </div>

              <div className="flex gap-4 text-sm font-mono">
                <span className="text-emerald-400">成功: {results.filter((r) => r.status === 'verified').length}</span>
                <span className="text-amber-400">重复: {results.filter((r) => r.status === 'duplicate').length}</span>
                <span className="text-red-400">失败: {results.filter((r) => r.status === 'failed').length}</span>
              </div>

              <div className="max-h-[300px] divide-y divide-white/10 overflow-y-auto rounded-md border border-white/10 bg-black/30">
                {results.map((result) => (
                  <div key={result.index} className="p-3">
                    <div className="flex items-start gap-3">
                      {getStatusIcon(result.status)}
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium text-neutral-200">{result.email || `凭据 #${result.index}`}</span>
                          <span className="text-xs text-neutral-400">{getStatusText(result)}</span>
                        </div>
                        {result.usage && <div className="mt-1 text-xs text-neutral-400">额度使用: {result.usage}</div>}
                        {result.error && <div className="mt-1 text-xs text-red-400">{result.error}</div>}
                        {result.rollbackError && (
                          <div className="mt-1 text-xs text-red-400">回滚错误: {result.rollbackError}</div>
                        )}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="secondary"
            onClick={() => {
              onOpenChange(false)
              resetForm()
            }}
            disabled={importing}
          >
            {importing ? (
              <span className="inline-flex items-center gap-2">
                <div className="orbital-loader scale-75" />
                验证中...
              </span>
            ) : results.length > 0 ? (
              '关闭'
            ) : (
              '取消'
            )}
          </Button>
          {results.length === 0 && (
            <Button type="button" onClick={handleBatchImport} disabled={importing || !jsonInput.trim()}>
              导入并验证
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
