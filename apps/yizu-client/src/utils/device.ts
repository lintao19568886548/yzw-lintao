const DEVICE_STORAGE_KEY = 'yizu-miniapp-device-v1'
const DEVICE_ID_PATTERN = /^[a-z0-9_-]{16,80}$/u

export function getOrCreateDeviceId(): string {
  const existing = String(uni.getStorageSync(DEVICE_STORAGE_KEY) || '').trim().toLowerCase()
  if (DEVICE_ID_PATTERN.test(existing)) return existing
  // This identifier is only a rate-limit dimension, not a credential.
  const created = `wx_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 18)}`
  uni.setStorageSync(DEVICE_STORAGE_KEY, created)
  return created
}
