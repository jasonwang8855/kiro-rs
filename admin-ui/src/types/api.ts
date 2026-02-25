export interface CredentialsStatusResponse {
  total: number
  available: number
  currentId: number
  credentials: CredentialStatusItem[]
}

export interface CredentialStatusItem {
  id: number
  priority: number
  disabled: boolean
  failureCount: number
  isCurrent: boolean
  expiresAt: string | null
  authMethod: string | null
  hasProfileArn: boolean
  email?: string
  refreshTokenHash?: string
  successCount: number
  lastUsedAt: string | null
  hasProxy: boolean
  proxyUrl?: string
}

export interface BalanceResponse {
  id: number
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  remaining: number
  usagePercentage: number
  nextResetAt: number | null
}

export interface SuccessResponse {
  success: boolean
  message: string
}

export interface AdminErrorResponse {
  error: {
    type: string
    message: string
  }
}

export interface SetDisabledRequest {
  disabled: boolean
}

export interface SetPriorityRequest {
  priority: number
}

export interface AddCredentialRequest {
  refreshToken: string
  authMethod?: 'social' | 'idc'
  clientId?: string
  clientSecret?: string
  priority?: number
  authRegion?: string
  apiRegion?: string
  machineId?: string
  proxyUrl?: string
  proxyUsername?: string
  proxyPassword?: string
}

export interface AddCredentialResponse {
  success: boolean
  message: string
  credentialId: number
  email?: string
}

export interface LoginRequest {
  username: string
  password: string
}

export interface LoginResponse {
  success: boolean
  token: string
  expiresAt: string
}

export interface ApiKeyItem {
  id: string
  name: string
  key: string
  enabled: boolean
  createdAt: string
  lastUsedAt: string | null
  requestCount: number
  inputTokens: number
  outputTokens: number
  keyPreview: string
}

export interface ApiKeyListResponse {
  keys: ApiKeyItem[]
}

export interface CreateApiKeyRequest {
  name: string
}

export interface CreateApiKeyResponse {
  success: boolean
  id: string
  name: string
  key: string
  keyPreview: string
}

export interface ApiUsageOverview {
  totalKeys: number
  enabledKeys: number
  totalRequests: number
  totalInputTokens: number
  totalOutputTokens: number
}

export interface ApiStatsResponse {
  overview: ApiUsageOverview
}

export interface TotalBalanceResponse {
  totalUsageLimit: number
  totalCurrentUsage: number
  totalRemaining: number
  credentialCount: number
}

export interface RequestLogEntry {
  id: string
  timestamp: string
  model: string
  stream: boolean
  messageCount: number
  inputTokens: number
  outputTokens: number
  tokenSource: string
  durationMs: number
  status: string
  apiKeyId: string
}

export interface RequestLogResponse {
  entries: RequestLogEntry[]
}
