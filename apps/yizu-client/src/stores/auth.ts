import { defineStore } from 'pinia'
import type { DevSessionResponse } from '@/types/domain'
import { readVersionedJson, removeStored, writeVersionedJson } from '@/utils/storage'

export const AUTH_STORAGE_KEY = 'yizu-miniapp-auth-v3'
const LEGACY_AUTH_STORAGE_KEYS = ['yizu-miniapp-demo-auth-v1', 'yizu-miniapp-demo-auth-v2']
const AUTH_STORAGE_VERSION = 3

interface AuthSnapshot {
  session_token: string
  refresh_token: string
  masked_phone: string
  expires_at_epoch_seconds: number
  local_demo: boolean
}

function isAuthSnapshot(value: unknown): value is AuthSnapshot {
  if (!value || typeof value !== 'object') return false
  const snapshot = value as Partial<AuthSnapshot>
  return typeof snapshot.session_token === 'string'
    && typeof snapshot.refresh_token === 'string'
    && typeof snapshot.masked_phone === 'string'
    && typeof snapshot.expires_at_epoch_seconds === 'number'
    && typeof snapshot.local_demo === 'boolean'
}

export const useAuthStore = defineStore('auth', {
  state: (): AuthSnapshot => ({
    session_token: '',
    refresh_token: '',
    masked_phone: '',
    expires_at_epoch_seconds: 0,
    local_demo: false,
  }),
  getters: {
    is_authenticated: (state): boolean =>
      state.session_token.length > 0 && state.expires_at_epoch_seconds > Math.floor(Date.now() / 1000),
  },
  actions: {
    setSession(session: DevSessionResponse): void {
      this.$patch({ ...session, refresh_token: session.refresh_token ?? '' })
      this.persist()
    },
    hydrate(): void {
      LEGACY_AUTH_STORAGE_KEYS.forEach(removeStored)
      const saved = readVersionedJson(AUTH_STORAGE_KEY, AUTH_STORAGE_VERSION, isAuthSnapshot)
      if (!saved) return
      this.$patch(saved)
      if (!this.is_authenticated) this.clear()
    },
    persist(): void {
      writeVersionedJson<AuthSnapshot>(AUTH_STORAGE_KEY, AUTH_STORAGE_VERSION, {
        session_token: this.session_token,
        refresh_token: this.refresh_token,
        masked_phone: this.masked_phone,
        expires_at_epoch_seconds: this.expires_at_epoch_seconds,
        local_demo: this.local_demo,
      })
    },
    clear(): void {
      this.$reset()
      removeStored(AUTH_STORAGE_KEY)
      LEGACY_AUTH_STORAGE_KEYS.forEach(removeStored)
    },
  },
})
