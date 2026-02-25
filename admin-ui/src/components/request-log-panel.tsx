import { useCallback, useEffect, useRef, useState } from 'react'
import { ChevronDown, ChevronRight, ScrollText, Trash2 } from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { getRequestLogs } from '@/api/credentials'
import type { RequestLogEntry } from '@/types/api'

interface RequestLogPanelProps {
  enabled: boolean
  onToggle: (enabled: boolean) => void
}

export function RequestLogPanel({ enabled, onToggle }: RequestLogPanelProps) {
  const [entries, setEntries] = useState<RequestLogEntry[]>([])
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set())
  const lastSeenIdRef = useRef<string | undefined>(undefined)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

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

  const handleClear = () => {
    setEntries([])
    setExpandedIds(new Set())
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
          <Switch checked={enabled} onCheckedChange={onToggle} />
        </div>
      </div>
      {enabled && (
        <div className="overflow-x-auto rounded-lg border border-white/10 bg-[#050505]">
          <table className="w-full min-w-[1000px] border-collapse">
            <thead>
              <tr className="border-b border-white/10 text-left text-xs text-neutral-500">
                <th className="w-6 px-2 py-2"></th>
                <th className="px-3 py-2 font-medium">时间</th>
                <th className="px-3 py-2 font-medium">模型</th>
                <th className="px-3 py-2 font-medium">流式</th>
                <th className="px-3 py-2 font-medium">消息数</th>
                <th className="px-3 py-2 font-medium">输入</th>
                <th className="px-3 py-2 font-medium">输出</th>
                <th className="px-3 py-2 font-medium">来源</th>
                <th className="px-3 py-2 font-medium">耗时</th>
                <th className="px-3 py-2 font-medium">状态</th>
              </tr>
            </thead>
            <tbody>
              {entries.length === 0 && (
                <tr>
                  <td colSpan={10} className="px-3 py-6 text-center text-sm text-neutral-600">
                    等待请求...
                  </td>
                </tr>
              )}
              {entries.map((e) => {
                const isExpanded = expandedIds.has(e.id)
                return (
                  <tr key={e.id} className="group">
                    <td colSpan={10} className="p-0">
                      <div
                        className="flex items-center border-b border-white/5 font-mono text-sm text-white cursor-pointer hover:bg-white/[0.02]"
                        onClick={() => toggleExpand(e.id)}
                      >
                        <div className="w-6 px-2 py-2 text-neutral-600 flex-shrink-0">
                          {isExpanded ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
                        </div>
                        <div className="px-3 py-2 text-neutral-400 text-xs min-w-[70px]">{formatTime(e.timestamp)}</div>
                        <div className="px-3 py-2 min-w-[140px]">
                          <Badge variant="secondary" className="text-xs font-mono">{e.model}</Badge>
                        </div>
                        <div className="px-3 py-2 min-w-[50px]">
                          {e.stream ? <span className="text-emerald-400 text-xs">SSE</span> : <span className="text-neutral-500 text-xs">JSON</span>}
                        </div>
                        <div className="px-3 py-2 text-neutral-300 min-w-[50px]">{e.messageCount}</div>
                        <div className="px-3 py-2 text-neutral-300 min-w-[80px]">{e.inputTokens.toLocaleString()}</div>
                        <div className="px-3 py-2 text-neutral-300 min-w-[80px]">{e.outputTokens.toLocaleString()}</div>
                        <div className="px-3 py-2 text-xs min-w-[50px]">
                          {e.tokenSource.includes('contextUsage') ? <span className="text-emerald-400">API</span> : <span className="text-amber-400">估算</span>}
                        </div>
                        <div className="px-3 py-2 text-neutral-300 min-w-[60px]">{(e.durationMs / 1000).toFixed(1)}s</div>
                        <div className="px-3 py-2 min-w-[50px]">
                          {e.status === 'success' ? <span className="text-emerald-400 text-xs">成功</span> : <span className="text-red-400 text-xs" title={e.status}>失败</span>}
                        </div>
                      </div>
                      {isExpanded && (
                        <div className="border-b border-white/5 bg-[#0a0a0a] px-4 py-3 space-y-3">
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
                      )}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  )
}
