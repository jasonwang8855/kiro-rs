import axios from 'axios'
import { storage } from '@/lib/storage'
import type {
  CredentialsStatusResponse,
  BalanceResponse,
  SuccessResponse,
  SetDisabledRequest,
  SetPriorityRequest,
  AddCredentialRequest,
  AddCredentialResponse,
  LoginRequest,
  LoginResponse,
  ApiKeyListResponse,
  CreateApiKeyRequest,
  CreateApiKeyResponse,
  ApiStatsResponse,
  TotalBalanceResponse,
  RequestLogResponse,
} from '@/types/api'

const api = axios.create({
  baseURL: '/api/admin',
  headers: {
    'Content-Type': 'application/json',
  },
})

api.interceptors.request.use((config) => {
  const token = storage.getToken()
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

export async function login(req: LoginRequest): Promise<LoginResponse> {
  const { data } = await api.post<LoginResponse>('/auth/login', req)
  return data
}

export async function getCredentials(): Promise<CredentialsStatusResponse> {
  const { data } = await api.get<CredentialsStatusResponse>('/credentials')
  return data
}

export async function setCredentialDisabled(
  id: number,
  disabled: boolean
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/disabled`,
    { disabled } as SetDisabledRequest
  )
  return data
}

export async function setCredentialPriority(
  id: number,
  priority: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(
    `/credentials/${id}/priority`,
    { priority } as SetPriorityRequest
  )
  return data
}

export async function resetCredentialFailure(
  id: number
): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/credentials/${id}/reset`)
  return data
}

export async function getCredentialBalance(id: number): Promise<BalanceResponse> {
  const { data } = await api.get<BalanceResponse>(`/credentials/${id}/balance`)
  return data
}

export async function addCredential(
  req: AddCredentialRequest
): Promise<AddCredentialResponse> {
  const { data } = await api.post<AddCredentialResponse>('/credentials', req)
  return data
}

export async function deleteCredential(id: number): Promise<SuccessResponse> {
  const { data } = await api.delete<SuccessResponse>(`/credentials/${id}`)
  return data
}

export async function getLoadBalancingMode(): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.get<{ mode: 'priority' | 'balanced' }>('/config/load-balancing')
  return data
}

export async function setLoadBalancingMode(mode: 'priority' | 'balanced'): Promise<{ mode: 'priority' | 'balanced' }> {
  const { data } = await api.put<{ mode: 'priority' | 'balanced' }>('/config/load-balancing', { mode })
  return data
}

export async function listApiKeys(): Promise<ApiKeyListResponse> {
  const { data } = await api.get<ApiKeyListResponse>('/apikeys')
  return data
}

export async function createApiKey(req: CreateApiKeyRequest): Promise<CreateApiKeyResponse> {
  const { data } = await api.post<CreateApiKeyResponse>('/apikeys', req)
  return data
}

export async function setApiKeyDisabled(id: string, disabled: boolean): Promise<SuccessResponse> {
  const { data } = await api.post<SuccessResponse>(`/apikeys/${id}/disabled`, { disabled })
  return data
}

export async function deleteApiKey(id: string): Promise<SuccessResponse> {
  const { data } = await api.delete<SuccessResponse>(`/apikeys/${id}`)
  return data
}

export async function getApiStats(): Promise<ApiStatsResponse> {
  const { data } = await api.get<ApiStatsResponse>('/stats')
  return data
}

export async function getTotalBalance(): Promise<TotalBalanceResponse> {
  const { data } = await api.get<TotalBalanceResponse>('/balance/total')
  return data
}

export async function exportCredentials(): Promise<unknown[]> {
  const { data } = await api.get<unknown[]>('/credentials/export')
  return data
}

export async function exportCredential(id: number): Promise<unknown> {
  const { data } = await api.get<unknown>(`/credentials/${id}/export`)
  return data
}

export async function getRequestLogs(sinceId?: string): Promise<RequestLogResponse> {
  const params = sinceId ? { since_id: sinceId } : {}
  const { data } = await api.get<RequestLogResponse>('/logs', { params })
  return data
}
