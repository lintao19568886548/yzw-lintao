import { defineConfig } from 'vite'
import uniModule from '@dcloudio/vite-plugin-uni'

// The current DCloud CLI package is CommonJS. Vite can expose it as either the
// callable default or a `{ default }` namespace depending on config bundling.
const uni = typeof uniModule === 'function'
  ? uniModule
  : (uniModule as unknown as { default: typeof uniModule }).default

export default defineConfig({
  plugins: [uni()],
})
