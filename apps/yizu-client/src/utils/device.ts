const DEVICE_STORAGE_KEY = 'yizu-miniapp-device-v1'
const DEVICE_ID_PATTERN = /^[a-z0-9_-]{16,80}$/u
let memoryDeviceId = ''

interface DeviceStorage {
  getStorageSync(key: string): unknown
  setStorageSync(key: string, value: string): void
}

function availableStorage(): DeviceStorage | null {
  if (typeof uni === 'undefined') return null
  if (typeof uni.getStorageSync !== 'function' || typeof uni.setStorageSync !== 'function') return null
  return uni
}

function createDeviceId(): string {
  const entropy = Math.random().toString(36).slice(2).padEnd(16, '0').slice(0, 16)
  return `wx_${Date.now().toString(36)}_${entropy}`
}

export function getOrCreateDeviceId(): string {
  if (DEVICE_ID_PATTERN.test(memoryDeviceId)) return memoryDeviceId

  const storage = availableStorage()
  try {
    const existing = String(storage?.getStorageSync(DEVICE_STORAGE_KEY) || '').trim().toLowerCase()
    if (DEVICE_ID_PATTERN.test(existing)) {
      memoryDeviceId = existing
      return existing
    }
  } catch {
    // Storage can be unavailable in privacy mode or during an early platform bootstrap.
  }

  // This random local identifier is only a rate-limit dimension, not a credential or device fingerprint.
  const created = createDeviceId()
  memoryDeviceId = created
  try {
    storage?.setStorageSync(DEVICE_STORAGE_KEY, created)
  } catch {
    // Keep the in-memory value so device storage failure never blocks login rendering or submission.
  }
  return created
}
