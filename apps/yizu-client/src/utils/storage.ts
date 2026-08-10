export function readJson<T>(key: string): T | null {
  try {
    const value = uni.getStorageSync(key) as unknown
    if (typeof value !== 'string' || value.length === 0) return null
    return JSON.parse(value) as T
  } catch {
    return null
  }
}

interface VersionedValue<T> {
  version: number
  data: T
}

export function readVersionedJson<T>(
  key: string,
  version: number,
  isCompatible: (value: unknown) => value is T,
): T | null {
  const saved = readJson<VersionedValue<unknown>>(key)
  if (!saved || saved.version !== version || !isCompatible(saved.data)) {
    removeStored(key)
    return null
  }
  return saved.data
}

export function writeVersionedJson<T>(key: string, version: number, data: T): void {
  writeJson<VersionedValue<T>>(key, { version, data })
}

export function writeJson<T>(key: string, value: T): void {
  uni.setStorageSync(key, JSON.stringify(value))
}

export function removeStored(key: string): void {
  uni.removeStorageSync(key)
}
