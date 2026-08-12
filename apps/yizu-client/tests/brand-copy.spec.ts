import { readFileSync, readdirSync } from 'node:fs'
import { extname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const repositoryRoot = fileURLToPath(new URL('../../..', import.meta.url))
const clientSource = join(repositoryRoot, 'apps/yizu-client/src')

function collectUserFacingSource(directory: string): string {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const entryPath = join(directory, entry.name)
      if (entry.isDirectory()) return collectUserFacingSource(entryPath)
      return ['.vue', '.ts', '.json'].includes(extname(entry.name)) ? readFileSync(entryPath, 'utf8') : ''
    })
    .join('\n')
}

describe('宜租网品牌定位文案', () => {
  const brandHeader = readFileSync(join(clientSource, 'components/BrandHeader.vue'), 'utf8')
  const home = readFileSync(join(clientSource, 'pages/home/index.vue'), 'utf8')
  const login = readFileSync(join(clientSource, 'pages/login/index.vue'), 'utf8')
  const profile = readFileSync(join(clientSource, 'pages/profile/index.vue'), 'utf8')
  const manifest = JSON.parse(readFileSync(join(clientSource, 'manifest.json'), 'utf8')) as { description: string }

  it('统一品牌定位、首页主标题、服务范围与信任文案', () => {
    expect(brandHeader).toContain('企业选址服务 · AI智能匹配')
    expect(brandHeader).toContain('真实房源 · 专业顾问 · 15分钟响应')
    expect(login).toContain('说出需求，AI帮您匹配合适空间')
    expect(login).toContain('厂房 · 仓库 · 写字楼')
    expect(home).toContain('厂房 · 仓库 · 写字楼')
  })

  it('完整平台介绍使用统一文案', () => {
    expect(manifest.description).toBe('宜租网——企业选址与空间租赁智能服务平台')
    expect(profile).toContain('宜租网——企业选址与空间租赁智能服务平台')
  })

  it('个人中心只展示用户可理解的服务状态', () => {
    expect(profile).toContain('专业顾问将协助核验您的找房需求')
    expect(profile).toContain('您的找房需求仅用于空间匹配与顾问服务')
    expect(profile).not.toMatch(/P0-\d+|BLOCKED|后续版本开放|待接入|MVP\s*·/)
  })

  it('用户界面不再出现旧地域品牌文案，但保留东莞业务范围', () => {
    expect(collectUserFacingSource(clientSource)).not.toContain('东莞产业空间')
    expect(home).toContain('东莞全市服务')
  })
})
