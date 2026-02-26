import { Fragment, useCallback, useEffect, useRef, useState } from 'react'
import { ChevronDown, ChevronRight, ScrollText, Trash2 } from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { getRequestLogs, getLogEnabled, setLogEnabled } from '@/api/credentials'
import type { RequestLogEntry } from '@/types/api'

export function RequestLogPanel() {
  const [enabled, setEnabled] = useState(false)
  const [entries, setEntries] = useState<RequestLogEntry[]>([])
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set())
  const lastSeenIdRef = useRef<string | undefined>(undefined)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  useEffect(() => {
    getLogEnabled().then((res) => setEnabled(res.enabled)).catch(() => {})
  }, [])

  const poll = useCallback(async () => {
    try {
      const res = await getRequestLogs(lastSeenIdRef.current)
      if (res.entries.length > 0) {
        lastSeenIdRef.current = res.entries[res.entries.length - 1].id
        setEntries((prev) => {
          const merged = [...res.entries.reverse(), ...prev]
          return merged.slice(0, 100)
        })
      }
    } catch {
      // ignore polling errors
    }
  }, [])

  useEffect(() => {
    if (enabled) {
      poll()
      intervalRef.current = setInterval(poll, 2000)
    } else if (intervalRef.current) {
      clearInterval(intervalRef.current)
      intervalRef.current = null
    }
    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current)
        intervalRef.current = null
      }
    }
  }, [enabled, poll])

  const handleToggle = async (value: boolean) => {
    setEnabled(value)
    if (!value) {
      setEntries([])
      setExpandedIds(new Set())
      lastSeenIdRef.current = undefined
    }
    try {
      await setLogEnabled(value)
    } catch {
      setEnabled(!value)
    }
  }

  const handleClear = () => {
    setEntries([])
    setExpandedIds(new Set())
    lastSeenIdRef.current = undefined
  }

  const toggleExpand = (id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const formatTime = (ts: string) => {
    try {
      const d = new Date(ts)
      return d.toLocaleTimeString('zh-CN', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })
    } catch {
      return ts
    }
  }

  const formatJson = (raw: string) => {
    try {
      return JSON.stringify(JSON.parse(raw), null, 2)
    } catch {
      return raw
    }
  }

  const formatKeyId = (keyId: string) => {
    if (!keyId) return '-'
    if (keyId.length <= 12) return keyId
    return keyId.slice(0, 6) + '…' + keyId.slice(-4)
  }

  return (
    <section className="col-span-1 md:col-span-12 mt-4">
      <div className="mb-4 flex items-center justify-between px-1">
        <h2 className="flex items-center gap-2 font-sans text-sm font-medium tracking-wide text-neutral-500">
          <ScrollText className="h-4 w-4" />
          请求日志
        </h2>
        <div className="flex items-center gap-3">
          {enabled && entries.length > 0 && (
            <Button size="sm" variant="ghost" className="h-6 px-2 text-xs text-neutral-500" onClick={handleClear}>
              <Trash2 className="mr-1 h-3 w-3" />
              清空
            </Button>
          )}
          <span className="text-xs font-mono text-neutral-500">
            {enabled ? `${entries.length} 条` : '已关闭'}
          </span>
          <Switch checked={enabled} onCheckedChange={handleToggle} />
        </div>
      </div>
      {enabled && (
        <div className="overflow-x-auto rounded-lg border border-white/10 bg-[#050505]">
          <table className="w-full min-w-[1100px] border-collapse table-fixed">
            <colgroup>
              <col className="w-[28px]" />
              <col className="w-[72px]" />
              <col className="w-[200px]" />
              <col className="w-[48px]" />
              <col className="w-[52px]" />
              <col className="w-[80px]" />
              <col className="w-[80px]" />
              <col className="w-[52px]" />
              <col className="w-[60px]" />
              <col className="w-[52px]" />
              <col />
            </colgroup>
            <thead>
              <tr className="border-b border-white/10 text-left text-xs text-neutral-500">
                <th className="px-2 py-2"></th>
                <th className="px-3 py-2 font-medium">时间</th>
                <th className="px-3 py-2 font-medium">模型</th>
                <th className="px-3 py-2 font-medium">流式</th>
                <th className="px-3 py-2 font-medium">消息</th>
                <th className="px-3 py-2 font-medium">输入</th>
                <th className="px-3 py-2 font-medium">输出</th>
                <th className="px-3 py-2 font-medium">来源</th>
                <th className="px-3 py-2 font-medium">耗时</th>
                <th className="px-3 py-2 font-medium">状态</th>
                <th className="px-3 py-2 font-medium">Key</th>
              </tr>
            </thead>
            <tbody>
              {entries.length === 0 && (
                <tr>
                  <td colSpan={11} className="px-3 py-6 text-center text-sm text-neutral-600">
                    等待请求...
                  </td>
                </tr>
              )}
              {entries.map((e) => {
                const isExpanded = expandedIds.has(e.id)
                return (
                  <Fragment key={e.id}>
                    <tr
                      className="border-b border-white/5 font-mono text-sm text-white cursor-pointer hover:bg-white/[0.02]"
                      onClick={() => toggleExpand(e.id)}
                    >
                      <td className="px-2 py-2 text-neutral-600">
                        {isExpanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
                      </td>
                      <td className="px-3 py-2 text-neutral-400 text-xs">{formatTime(e.timestamp)}</td>
                      <td className="px-3 py-2 truncate">
                        <Badge variant="secondary" className="text-xs font-mono">{e.model}</Badge>
                      </td>
                      <td className="px-3 py-2">
                        {e.stream ? <span className="text-emerald-400 text-xs">SSE</span> : <span className="text-neutral-500 text-xs">JSON</span>}
                      </td>
                      <td className="px-3 py-2 text-neutral-300">{e.messageCount}</td>
                      <td className="px-3 py-2 text-neutral-300">{e.inputTokens.toLocaleString()}</td>
                      <td className="px-3 py-2 text-neutral-300">{e.outputTokens.toLocaleString()}</td>
                      <td className="px-3 py-2 text-xs">
                        {e.tokenSource.includes('contextUsage') ? <span className="text-emerald-400">API</span> : <span className="text-amber-400">估算</span>}
                      </td>
                      <td className="px-3 py-2 text-neutral-300">{(e.durationMs / 1000).toFixed(1)}s</td>
                      <td className="px-3 py-2">
                        {e.status === 'success' ? <span className="text-emerald-400 text-xs">成功</span> : <span className="text-red-400 text-xs" title={e.status}>失败</span>}
                      </td>
                      <td className="px-3 py-2 text-neutral-500 text-xs truncate" title={e.apiKeyId}>{formatKeyId(e.apiKeyId)}</td>
                    </tr>
                    {isExpanded && (
                      <tr key={`${e.id}-detail`} className="border-b border-white/5">
                        <td colSpan={11} className="bg-[#0a0a0a] px-4 py-3">
                          <div className="space-y-3">
                            <div>
                              <div className="text-xs text-neutral-500 mb-1">请求内容</div>
                              <pre className="text-xs text-neutral-300 bg-[#111] rounded p-3 overflow-x-auto max-h-[400px] overflow-y-auto whitespace-pre-wrap break-all">
                                {e.requestBody ? formatJson(e.requestBody) : '(无)'}
                              </pre>
                            </div>
                            <div>
                              <div className="text-xs text-neutral-500 mb-1">回复内容</div>
                              <pre className="text-xs text-neutral-300 bg-[#111] rounded p-3 overflow-x-auto max-h-[400px] overflow-y-auto whitespace-pre-wrap break-all">
                                {e.responseBody || '(无)'}
                              </pre>
                            </div>
                          </div>
                        </td>
                      </tr>
                    )}
                  </Fragment>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}
