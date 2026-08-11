/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_YIZU_API_BASE_URL?: string
  readonly VITE_YIZU_DEMO_MODE?: string
  readonly VITE_YIZU_MODE?: 'demo' | 'test' | 'production'
  readonly VITE_YIZU_PUBLIC_H5_BASE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
