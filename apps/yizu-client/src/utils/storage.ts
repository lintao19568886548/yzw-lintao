export function readJson<T>(key: string): T | null {
  try {
    const value = uni.getStorageSync(key) as unknown
    if (typeof value !== 'string' || value.length === 0) return null
    return JSON.parse(value) as T
  } catch {
    return null
  }
}

export function writeJson<T>(key: string, value: T): void {
  uni.setStorageSync(key, JSON.stringify(value))
}

export function removeStored(key: string): void {
  uni.removeStorageSync(key)
}
