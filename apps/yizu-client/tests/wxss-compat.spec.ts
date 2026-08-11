import { describe, expect, it } from 'vitest'
// @ts-expect-error Native Node ESM checker intentionally has no TypeScript declaration file.
import { findIncompatibleWxss } from '../scripts/check-mp-weixin-wxss.mjs'

describe('微信 WXSS 构建产物检查', () => {
  it('拦截裸通配选择器并返回位置', () => {
    const findings = findIncompatibleWxss('app.wxss', '.row>*{flex:1}\n*,view{box-sizing:border-box}')

    expect(findings).toHaveLength(2)
    expect(findings[0]).toMatchObject({
      filePath: 'app.wxss',
      line: 1,
      column: 1,
      code: 'universal-selector',
      selector: '.row>*',
    })
    expect(findings[1]).toMatchObject({ line: 2, column: 1, code: 'universal-selector' })
  })

  it('不误判注释、字符串、属性包含匹配和动画百分比', () => {
    const wxss = `
      /* *, *::before { box-sizing: border-box; } */
      [data-kind*="factory"] { content: "*"; }
      @keyframes pulse { 0% { opacity: 0; } 100% { opacity: 1; } }
      .value { width: calc(2px * 3); }
    `

    expect(findIncompatibleWxss('safe.wxss', wxss)).toEqual([])
  })

  it('拦截 H5 根选择器和浏览器专用规则', () => {
    const wxss = 'html body{margin:0}:root{color:red}.card:has(view){display:block}@supports(display:grid){.grid{display:grid}}'
    const codes = findIncompatibleWxss('browser.wxss', wxss).map((finding: { code: string }) => finding.code)

    expect(codes).toEqual(expect.arrayContaining([
      'browser-root-selector',
      'browser-only-pseudo',
      'supports-at-rule',
    ]))
  })
})
