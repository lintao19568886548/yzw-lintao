import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const homeSource = readFileSync(
  fileURLToPath(new URL('../src/pages/home/index.vue', import.meta.url)),
  'utf8',
)
const globalStyles = readFileSync(
  fileURLToPath(new URL('../src/styles/global.css', import.meta.url)),
  'utf8',
)
const homeTemplate = homeSource.match(/<template>([\s\S]*?)<\/template>/u)?.[1] ?? ''

function cssRule(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')
  return globalStyles.match(new RegExp(`^${escaped}\\s*\\{([^}]*)\\}`, 'mu'))?.[1] ?? ''
}

describe('微信原生输入框文字完整显示', () => {
  it('首页三个基础条件使用外层视觉容器和无上下内边距的原生 input', () => {
    expect(homeTemplate.match(/class="field-control"/gu)).toHaveLength(3)
    expect(homeTemplate.match(/class="field-input"/gu)).toHaveLength(3)
    expect(homeTemplate.match(/placeholder-class="field-placeholder"/gu)).toHaveLength(3)
    expect(homeTemplate).toContain('面积下限（㎡）')
    expect(homeTemplate).toContain('面积上限（㎡）')
    expect(homeTemplate).toContain('月租预算（元）')
  })

  it('输入层高度、字号和行高稳定且不使用垂直 padding', () => {
    const fieldControl = cssRule('.field-control')
    const fieldInput = cssRule('.field-input')
    const sharedInput = cssRule('.input')

    expect(fieldControl).toMatch(/display:\s*flex/u)
    expect(fieldControl).toMatch(/align-items:\s*center/u)
    expect(fieldControl).toMatch(/min-height:\s*92rpx/u)
    expect(fieldInput).toMatch(/height:\s*88rpx/u)
    expect(fieldInput).toMatch(/font-size:\s*28rpx/u)
    expect(fieldInput).toMatch(/line-height:\s*normal/u)
    expect(fieldInput).toMatch(/padding:\s*0(?:;|\s)/u)
    expect(sharedInput).toMatch(/height:\s*88rpx/u)
    expect(sharedInput).toMatch(/padding:\s*0 23rpx/u)
  })

  it('占位文字对比清晰，聚焦规则不改变输入框几何尺寸', () => {
    expect(cssRule('.field-placeholder')).toMatch(/color:\s*#8f877f/u)
    expect(cssRule('.field-placeholder')).toMatch(/font-size:\s*28rpx/u)
    expect(cssRule('.field-input:focus')).not.toMatch(/(?:height|padding|line-height):/u)
  })

  it('面积字段保持等宽双列且小屏允许子项收缩', () => {
    expect(homeTemplate).toContain('class="row field area-fields"')
    expect(globalStyles).toMatch(/\.row\s*>\s*view\s*\{[^}]*flex:\s*1;[^}]*min-width:\s*0;/u)
    expect(homeSource).toMatch(/\.area-fields\s*>\s*view\s*\{\s*min-width:\s*0;/u)
  })
})
