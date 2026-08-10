export function installStorageMock(initial: Record<string, string> = {}): Map<string, string> {
  const storage = new Map(Object.entries(initial))
  const uniMock = {
    getStorageSync: (key: string): string => storage.get(key) ?? '',
    setStorageSync: (key: string, value: string): void => { storage.set(key, value) },
    removeStorageSync: (key: string): void => { storage.delete(key) },
  }
  Object.assign(globalThis, { uni: uniMock })
  return storage
}
