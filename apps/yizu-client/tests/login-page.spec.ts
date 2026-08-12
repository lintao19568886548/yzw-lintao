import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import {
  agreementFromCheckboxEvent,
  canSubmitLogin,
  completeDemoLogin,
  phoneFromInputEvent,
} from '@/pages/login/model'

const loginPageSource = readFileSync(
  fileURLToPath(new URL('../src/pages/login/index.vue', import.meta.url)),
  'utf8',
)
const loginTemplate = loginPageSource.match(/<template>([\s\S]*?)<\/template>/u)?.[1] ?? ''

describe('登录页状态与演示登录', () => {
  const phone = ['139', '0000', '0000'].join('')

  it('从微信 input 的 event.detail.value 写入手机号', () => {
    expect(phoneFromInputEvent({ detail: { value: phone } })).toBe(phone)
  })

  it('协议真实状态与微信 checkbox 事件一致', () => {
    expect(agreementFromCheckboxEvent({ detail: { value: ['accepted'] } })).toBe(true)
    expect(agreementFromCheckboxEvent({ detail: { value: [] } })).toBe(false)
  })

  it('唯一启用条件是合法手机号、同意协议且未提交', () => {
    expect(canSubmitLogin(phone, true, false)).toBe(true)
    expect(canSubmitLogin('123', true, false)).toBe(false)
    expect(canSubmitLogin(phone, false, false)).toBe(false)
    expect(canSubmitLogin(phone, true, true)).toBe(false)
  })

  it('演示登录保存会话后跳转 AI 找房首页', async () => {
    const saveSession = vi.fn()
    const navigateToAiHome = vi.fn()
    const session = {
      session_token: 'local_demo_test', masked_phone: '139****0000',
      expires_at_epoch_seconds: 2_000_000_000, local_demo: true,
    }
    await completeDemoLogin(phone, true, {
      createSession: vi.fn().mockResolvedValue(session), saveSession, navigateToAiHome,
    })
    expect(saveSession).toHaveBeenCalledWith(session)
    expect(navigateToAiHome).toHaveBeenCalledOnce()
  })

  it('未同意协议时不调用登录接口', async () => {
    const createSession = vi.fn()
    await expect(completeDemoLogin(phone, false, {
      createSession, saveSession: vi.fn(), navigateToAiHome: vi.fn(),
    })).rejects.toThrow(/用户协议/u)
    expect(createSession).not.toHaveBeenCalled()
  })

  it('登录失败向上抛出供页面显示，不静默吞掉', async () => {
    await expect(completeDemoLogin(phone, true, {
      createSession: vi.fn().mockRejectedValue(new Error('模拟登录失败')),
      saveSession: vi.fn(), navigateToAiHome: vi.fn(),
    })).rejects.toThrow('模拟登录失败')
  })

  it('渲染新的品牌层级、服务范围与信任信息', () => {
    expect(loginTemplate).toContain('宜租网')
    expect(loginTemplate).toContain('企业选址服务 · AI智能匹配')
    expect(loginTemplate).toContain('说出需求，AI帮您匹配合适空间')
    expect(loginTemplate).toContain('厂房 · 仓库 · 写字楼')
    expect(loginTemplate).toContain('真实房源｜专业顾问｜15分钟响应')
    expect(loginTemplate).toContain('宜租网——企业选址与空间租赁智能服务平台')
  })

  it('演示标签只受 localDemoMode 控制且不暴露开发计划', () => {
    expect(loginTemplate).toMatch(/v-if="localDemoMode" class="demo-badge">本地演示 · 不发送短信/u)
    expect(loginTemplate).not.toContain('后续登录能力')
    expect(loginTemplate).not.toContain('微信手机号一键登录')
    expect(loginTemplate).not.toContain('企业实名认证')
    expect(loginTemplate).not.toContain('即将接入')
  })

  it('协议只展示一次且控件仍绑定真实状态', () => {
    expect(loginTemplate.match(/《用户协议》/gu)).toHaveLength(1)
    expect(loginTemplate.match(/《隐私政策》/gu)).toHaveLength(1)
    expect(loginTemplate).toContain(':checked="agreed"')
    expect(loginTemplate).toContain('@change="onAgreementChange"')
  })

  it('视觉重构保留手机号输入、提交条件和登录处理器绑定', () => {
    expect(loginTemplate).toContain(':value="phone"')
    expect(loginTemplate).toContain('@input="onPhoneInput"')
    expect(loginTemplate).toContain(':disabled="!canSubmit"')
    expect(loginTemplate).toContain('@click="login"')
  })

  it('已登录启动跳转等待页面 ready，避免在 onLoad 阶段抢占 tabBar 初始化', () => {
    expect(loginPageSource).toMatch(/onLoad\(\(\) => \{\s+auth\.hydrate\(\)\s+\}\)/u)
    expect(loginPageSource).toMatch(/onReady\(\(\) => \{\s+if \(auth\.is_authenticated\) goHome\(\)\s+\}\)/u)
  })
})
