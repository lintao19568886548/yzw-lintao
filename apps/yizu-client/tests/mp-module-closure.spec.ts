import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
// @ts-expect-error Native Node ESM checker intentionally has no TypeScript declaration file.
import { analyzeModuleSources, extractRelativeRequires } from '../scripts/check-mp-weixin-modules.mjs'
// @ts-expect-error Native Node ESM publisher intentionally has no TypeScript declaration file.
import { publicationPlan } from '../scripts/build-mp-weixin.mjs'

const repositoryRoot = fileURLToPath(new URL('../../..', import.meta.url))

describe('微信构建模块依赖完整性', () => {
  const completeSources = new Map([
    ['pages/login/index.js', `require('../../api/miniapp.js');require('../../api/client.js')`],
    ['api/miniapp.js', `require('./client.js')`],
    ['api/client.js', `require('../utils/device.js');require('../common/vendor.js')`],
    ['utils/device.js', `require('../common/vendor.js')`],
    ['common/vendor.js', 'module.exports={}'],
  ])

  it('api/client 的设备工具和登录页依赖闭包可完整解析', () => {
    const result = analyzeModuleSources(completeSources)

    expect(result.missing).toEqual([])
    expect(result.entryClosures.get('pages/login/index.js')).toEqual([
      'api/client.js',
      'api/miniapp.js',
      'common/vendor.js',
      'pages/login/index.js',
      'utils/device.js',
    ])
  })

  it('缺少 device.js 时精确报告引用文件和缺失路径', () => {
    const withoutDevice = new Map(completeSources)
    withoutDevice.delete('utils/device.js')
    const result = analyzeModuleSources(withoutDevice)

    expect(result.missing).toEqual(expect.arrayContaining([
      expect.objectContaining({ importer: 'api/client.js', request: '../utils/device.js' }),
    ]))
  })

  it('忽略微信/第三方非相对模块并支持目录 index 与 JSON', () => {
    const sources = new Map([
      ['pages/login/index.js', `require('plugin://provider/runtime');require('../../config');require('../../locale.json')`],
      ['config/index.js', 'module.exports={}'],
      ['locale.json', '{}'],
    ])

    expect(analyzeModuleSources(sources).missing).toEqual([])
    expect(extractRelativeRequires(`require('weixin-runtime');require('./local.js')`)).toEqual([
      expect.objectContaining({ request: './local.js' }),
    ])
  })

  it('源码设备工具存在且使用无扩展名导入', () => {
    const devicePath = `${repositoryRoot}/apps/yizu-client/src/utils/device.ts`
    const client = readFileSync(`${repositoryRoot}/apps/yizu-client/src/api/client.ts`, 'utf8')

    expect(existsSync(devicePath)).toBe(true)
    expect(client).toContain("from '@/utils/device'")
    expect(client).not.toMatch(/utils\/device\.js['"]/u)
  })

  it('安全发布时先写入被依赖模块并最后用 app.json 触发重载', () => {
    const analysis = analyzeModuleSources(completeSources)
    const plan = publicationPlan([...completeSources.keys(), 'app.json', 'app.wxss'], analysis.graph)

    expect(plan[0]).toBe('app.json')
    expect(plan.at(-1)).toBe('app.json')
    expect(plan.indexOf('utils/device.js')).toBeLessThan(plan.indexOf('api/client.js'))
    expect(plan.indexOf('api/client.js')).toBeLessThan(plan.indexOf('api/miniapp.js'))
    expect(plan.indexOf('api/miniapp.js')).toBeLessThan(plan.indexOf('pages/login/index.js'))
  })
})
