const ADMIN_TOKEN_STORAGE_KEY = 'adminSessionToken'

export const storage = {
  getToken: () => localStorage.getItem(ADMIN_TOKEN_STORAGE_KEY),
  setToken: (token: string) => localStorage.setItem(ADMIN_TOKEN_STORAGE_KEY, token),
  removeToken: () => localStorage.removeItem(ADMIN_TOKEN_STORAGE_KEY),
}
