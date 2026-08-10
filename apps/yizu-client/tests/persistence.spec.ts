import { createPinia, setActivePinia } from 'pinia'
import { useAuthStore } from '@/stores/auth'
import { installStorageMock } from './test-helpers'

describe('Pinia 状态恢复', () => {
  it('刷新后恢复未过期的本地会话', () => {
    const storage = installStorageMock()
    setActivePinia(createPinia())
    const first = useAuthStore()
    first.setSession({
      session_token: 'demo-session',
      masked_phone: '139****5678',
      expires_at_epoch_seconds: Math.floor(Date.now() / 1000) + 3600,
      local_demo: true,
    })
    expect(storage.size).toBe(1)

    setActivePinia(createPinia())
    const restored = useAuthStore()
    restored.hydrate()
    expect(restored.is_authenticated).toBe(true)
    expect(restored.masked_phone).toBe('139****5678')
  })
})
