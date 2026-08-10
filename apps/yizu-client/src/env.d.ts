/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_YIZU_API_BASE_URL?: string
  readonly VITE_YIZU_DEMO_MODE?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
