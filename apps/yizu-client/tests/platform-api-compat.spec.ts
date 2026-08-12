import { readFileSync, readdirSync } from 'node:fs'
import { extname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const repositoryRoot = fileURLToPath(new URL('../../..', import.meta.url))
const clientSource = join(repositoryRoot, 'apps/yizu-client/src')
const sourceExtensions = new Set(['.js', '.jsx', '.ts', '.tsx', '.vue'])

interface SourceFile {
  path: string
  content: string
}

function collectSourceFiles(directory: string): SourceFile[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = join(directory, entry.name)
    if (entry.isDirectory()) return collectSourceFiles(entryPath)
    if (!sourceExtensions.has(extname(entry.name))) return []
    return [{ path: relative(clientSource, entryPath), content: readFileSync(entryPath, 'utf8') }]
  })
}

describe('微信与 HarmonyOS 平台 API 兼容性', () => {
  it('客户端业务源码不直接调用已废弃的 getSystemInfo 系列 API', () => {
    const deprecatedCall = /\b(?:uni|wx)\s*\.\s*getSystemInfo(?:Sync)?\s*\(/
    const violations = collectSourceFiles(clientSource)
      .filter(({ content }) => deprecatedCall.test(content))
      .map(({ path }) => path)

    expect(violations, `发现已废弃设备 API：${violations.join(', ')}`).toEqual([])
  })
})
