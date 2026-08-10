import { defineStore } from 'pinia'
import type { DevSessionResponse } from '@/types/domain'
import { readJson, removeStored, writeJson } from '@/utils/storage'

const AUTH_STORAGE_KEY = 'yizu-miniapp-demo-auth-v1'

interface AuthSnapshot {
  session_token: string
  masked_phone: string
  expires_at_epoch_seconds: number
  local_demo: boolean
}

export const useAuthStore = defineStore('auth', {
  state: (): AuthSnapshot => ({
    session_token: '',
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
      this.$patch(session)
      this.persist()
    },
    hydrate(): void {
      const saved = readJson<AuthSnapshot>(AUTH_STORAGE_KEY)
      if (!saved) return
      this.$patch(saved)
      if (!this.is_authenticated) this.clear()
    },
    persist(): void {
      writeJson<AuthSnapshot>(AUTH_STORAGE_KEY, {
        session_token: this.session_token,
        masked_phone: this.masked_phone,
        expires_at_epoch_seconds: this.expires_at_epoch_seconds,
        local_demo: this.local_demo,
      })
    },
    clear(): void {
      this.$reset()
      removeStored(AUTH_STORAGE_KEY)
    },
  },
})
