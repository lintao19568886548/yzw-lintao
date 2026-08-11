<script setup lang="ts">
import { computed, ref } from 'vue'
import { onLoad } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { localDemoMode } from '@/config/runtime'
import { useAuthStore } from '@/stores/auth'
import { getOrCreateDeviceId } from '@/utils/device'
import { goHome } from '@/utils/navigation'
import { isValidPhone } from '@/utils/validation'
import {
  agreementFromCheckboxEvent,
  canSubmitLogin,
  codeFromInputEvent,
  completeDemoLogin,
  phoneFromInputEvent,
} from './model'

const auth = useAuthStore()
const phone = ref('')
const agreed = ref(false)
const verificationCode = ref('')
const sendingCode = ref(false)
const smsHint = ref('')
const loading = ref(false)
const errorMessage = ref('')
const canSubmit = computed(() => canSubmitLogin(phone.value, agreed.value, loading.value))

onLoad(() => {
  auth.hydrate()
  if (auth.is_authenticated) goHome()
})

function onPhoneInput(event: Event): void {
  phone.value = phoneFromInputEvent(event)
}

function onAgreementChange(event: Event): void {
  agreed.value = agreementFromCheckboxEvent(event)
}

function onCodeInput(event: Event): void {
  verificationCode.value = codeFromInputEvent(event)
}

async function sendSmsCode(): Promise<void> {
  errorMessage.value = ''
  smsHint.value = ''
  if (localDemoMode) return
  if (!isValidPhone(phone.value)) { errorMessage.value = '请输入有效的11位中国大陆手机号'; return }
  if (!agreed.value) { errorMessage.value = '请先阅读并同意协议'; return }
  if (sendingCode.value) return
  sendingCode.value = true
  try {
    const result = await miniappApi.sendSmsCode({ phone: phone.value, device_id: getOrCreateDeviceId() })
    smsHint.value = `验证码已发送，${result.expires_in_seconds}秒内有效`
  } catch (error) {
    errorMessage.value = mapApiError(error)
  } finally {
    sendingCode.value = false
  }
}

async function login(): Promise<void> {
  if (loading.value) return
  errorMessage.value = ''
  if (!isValidPhone(phone.value)) { errorMessage.value = '请输入有效的11位中国大陆手机号'; return }
  if (!agreed.value) { errorMessage.value = '请阅读并同意《用户协议》与《隐私政策》'; return }
  loading.value = true
  try {
    if (localDemoMode) {
      await completeDemoLogin(phone.value, agreed.value, {
        createSession: (request) => miniappApi.createDevSession(request),
        saveSession: (session) => auth.setSession(session),
        navigateToAiHome: goHome,
      })
    } else {
      if (!/^\d{4,8}$/u.test(verificationCode.value)) throw new Error('请输入短信验证码')
      auth.setSession(await miniappApi.verifySmsCode({
        phone: phone.value,
        code: verificationCode.value,
        device_id: getOrCreateDeviceId(),
        agreements_accepted: agreed.value,
      }))
      goHome()
    }
  } catch (error) {
    errorMessage.value = mapApiError(error)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <view class="login-page">
    <BrandHeader />
    <view class="page-shell content-width">
      <view class="welcome">
        <DemoBadge v-if="localDemoMode" />
        <text class="page-kicker">企业找房入口</text>
        <text class="page-title">让找产业空间，像说一句话一样简单</text>
        <text class="page-desc">登录后描述需求，AI 将整理条件、推荐已核验房源，并连接专业招商顾问。</text>
      </view>
      <view class="card login-card">
        <text class="section-title">手机号登录</text>
        <view class="field"><text class="field-label">联系手机号</text><input :value="phone" class="input" type="number" maxlength="11" placeholder="请输入11位手机号" @input="onPhoneInput" /></view>
        <view v-if="!localDemoMode" class="field code-field">
          <text class="field-label">短信验证码</text>
          <view class="code-row">
            <input :value="verificationCode" class="input code-input" type="number" maxlength="8" placeholder="请输入验证码" @input="onCodeInput" />
            <button class="code-button" :disabled="sendingCode" @click="sendSmsCode">{{ sendingCode ? '发送中…' : '获取验证码' }}</button>
          </view>
          <text v-if="smsHint" class="success-hint">{{ smsHint }}</text>
        </view>
        <checkbox-group class="confirm-row" @change="onAgreementChange">
          <label class="agreement-label">
            <checkbox value="accepted" :checked="agreed" color="#9b1d26" />
            <text>我已阅读并同意《用户协议》与《隐私政策》，并确认该号码可用于顾问联系</text>
          </label>
        </checkbox-group>
        <view v-if="localDemoMode" class="notice">演示登录仅保存在本机，不发送短信，也不会通知真实顾问。</view>
        <view v-else class="notice">真实服务模式通过 Rust BFF 发送并验证短信，服务密钥不会进入小程序。</view>
        <view v-if="errorMessage" class="error-panel">{{ errorMessage }}</view>
        <button class="primary-button" :disabled="!canSubmit" @click="login">{{ loading ? '正在安全登录…' : '登录并开始 AI 找房' }}</button>
      </view>
      <view class="future-login">
        <text class="future-title">后续登录能力</text>
        <view><text>微信手机号一键登录</text><text>即将接入</text></view>
        <view><text>企业实名认证</text><text>即将接入</text></view>
      </view>
      <text class="privacy-copy">继续即表示你已阅读并同意《用户协议》与《隐私政策》</text>
    </view>
  </view>
</template>

<style scoped>
.login-page { min-height: 100vh; background: radial-gradient(circle at 85% 8%, #f1d89e 0, transparent 32%), #f6f3ed; }
.welcome { padding: 12rpx 6rpx 28rpx; }.welcome .page-kicker { margin-top: 22rpx; }
.login-card { padding: 34rpx; }.confirm-row { display: block; margin-top: 24rpx; color: #654d43; font-size: 24rpx; line-height: 1.5; }.agreement-label { display: flex; align-items: flex-start; gap: 12rpx; }
.code-field { margin-top: 20rpx; }.code-row { display: flex; gap: 14rpx; }.code-input { flex: 1; }.code-button { width: 210rpx; margin: 0; font-size: 24rpx; }.success-hint { display: block; margin-top: 10rpx; color: #32734a; font-size: 22rpx; }
.future-login { margin-top: 26rpx; padding: 26rpx; border: 1rpx dashed #d9c8a9; border-radius: 22rpx; background: rgba(255,253,247,.65); }
.future-title { display: block; margin-bottom: 10rpx; color: #6f4a2b; font-size: 24rpx; font-weight: 800; }
.future-login view { display: flex; justify-content: space-between; padding: 13rpx 0; color: #78665f; font-size: 22rpx; }.future-login view text:last-child { color: #a27a3f; }
.privacy-copy { display: block; margin: 26rpx 20rpx; text-align: center; color: #98877f; font-size: 20rpx; }
</style>
