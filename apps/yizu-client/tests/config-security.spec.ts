import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const repositoryRoot = fileURLToPath(new URL('../../..', import.meta.url))

describe('客户端配置安全边界', () => {
  it('客户端环境类型只声明公开配置', () => {
    const envTypes = readFileSync(`${repositoryRoot}/apps/yizu-client/src/env.d.ts`, 'utf8')
    expect(envTypes).not.toMatch(/VITE_.*(?:APP_SECRET|SECRET_KEY|API_KEY|DATABASE_URL)/u)
  })

  it('微信 manifest 不包含 AppSecret 字段', () => {
    const manifest = readFileSync(`${repositoryRoot}/apps/yizu-client/src/manifest.json`, 'utf8')
    expect(manifest.toLowerCase()).not.toContain('appsecret')
  })

  it('本地私密配置被 Git 忽略而示例文件可跟踪', () => {
    const gitignore = readFileSync(`${repositoryRoot}/.gitignore`, 'utf8')
    expect(gitignore).toContain('.env.*')
    expect(gitignore).toContain('!.env.example')
    expect(gitignore).toContain('*.local')
  })
})
