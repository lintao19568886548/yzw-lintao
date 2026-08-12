<script setup lang="ts">
import { computed, ref } from 'vue'
import { onLoad, onReady } from '@dcloudio/uni-app'
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
})

onReady(() => {
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
    <view class="login-hero">
      <view class="hero-orbit hero-orbit-one" />
      <view class="hero-orbit hero-orbit-two" />
      <view class="hero-inner content-width">
        <view class="brand-line">
          <view class="login-seal"><text>宜</text><text>租</text></view>
          <view class="brand-copy">
            <text class="brand-name">宜租网</text>
            <text class="brand-positioning">企业选址服务 · AI智能匹配</text>
          </view>
        </view>
        <view class="hero-copy">
          <text class="hero-kicker">企业选址入口</text>
          <text class="hero-title">说出需求，AI帮您匹配合适空间</text>
          <view class="service-scope"><view class="scope-dot" /><text>厂房 · 仓库 · 写字楼</text></view>
        </view>
      </view>
    </view>

    <view class="login-main content-width">
      <view class="card login-card">
        <view class="card-heading">
          <text class="card-title">验证手机号，开始AI找房</text>
          <view v-if="localDemoMode" class="demo-badge">本地演示 · 不发送短信</view>
        </view>

        <view class="login-field">
          <text class="field-label">联系手机号</text>
          <input
            :value="phone"
            class="phone-input"
            type="number"
            maxlength="11"
            placeholder="请输入11位手机号"
            placeholder-style="color: #a3978e"
            confirm-type="done"
            cursor-spacing="24"
            @input="onPhoneInput"
          />
        </view>

        <view v-if="!localDemoMode" class="field code-field">
          <text class="field-label">短信验证码</text>
          <view class="code-row">
            <input :value="verificationCode" class="code-input" type="number" maxlength="8" placeholder="请输入验证码" placeholder-style="color: #a3978e" cursor-spacing="24" @input="onCodeInput" />
            <button class="code-button" :disabled="sendingCode" hover-class="code-button--pressed" @click="sendSmsCode">{{ sendingCode ? '发送中…' : '获取验证码' }}</button>
          </view>
          <text v-if="smsHint" class="success-hint">{{ smsHint }}</text>
        </view>

        <view v-if="errorMessage" class="error-panel">{{ errorMessage }}</view>
        <button class="primary-button login-submit" :disabled="!canSubmit" hover-class="login-submit--pressed" @click="login">{{ loading ? '正在安全登录…' : '登录并开始AI找房' }}</button>

        <checkbox-group class="agreement-group" @change="onAgreementChange">
          <label class="agreement-label">
            <checkbox class="agreement-checkbox" value="accepted" :checked="agreed" color="#A9151D" />
            <text class="agreement-copy">已阅读并同意<text class="agreement-link">《用户协议》</text>和<text class="agreement-link">《隐私政策》</text></text>
          </label>
        </checkbox-group>
      </view>

      <text class="trust-copy">真实房源｜专业顾问｜15分钟响应</text>
      <text class="platform-copy">宜租网——企业选址与空间租赁智能服务平台</text>
    </view>
  </view>
</template>

<style scoped>
.login-page {
  min-height: 100vh;
  overflow-x: hidden;
  background: #fff9ef;
  color: #2a211b;
}

.login-hero {
  position: relative;
  overflow: hidden;
  min-height: 430rpx;
  padding-top: calc(env(safe-area-inset-top) + 28rpx);
  border-radius: 0 0 50rpx 50rpx;
  background: linear-gradient(145deg, #7f1016 0%, #a9151d 64%, #95151c 100%);
  color: #fff9ef;
}

.hero-inner {
  position: relative;
  z-index: 2;
  max-width: 720rpx;
  padding: 18rpx 36rpx 84rpx;
}

.hero-orbit {
  position: absolute;
  border: 2rpx solid rgba(243, 217, 154, .13);
  border-radius: 50%;
}

.hero-orbit-one { top: 44rpx; right: -108rpx; width: 318rpx; height: 318rpx; }
.hero-orbit-two { bottom: -156rpx; left: -122rpx; width: 286rpx; height: 286rpx; }

.brand-line { display: flex; align-items: center; gap: 20rpx; }

.login-seal {
  display: grid;
  flex: 0 0 78rpx;
  width: 78rpx;
  height: 78rpx;
  grid-template-columns: 1fr 1fr;
  place-items: center;
  padding: 9rpx;
  border: 3rpx double #f3d99a;
  border-radius: 18rpx;
  background: rgba(78, 5, 11, .2);
  color: #fff1c9;
  font-family: "STKaiti", "KaiTi", serif;
  font-size: 25rpx;
  font-weight: 900;
  transform: rotate(-2deg);
}

.brand-copy { min-width: 0; }
.brand-name { display: block; font-size: 40rpx; font-weight: 900; letter-spacing: 4rpx; line-height: 1.15; }
.brand-positioning { display: block; margin-top: 7rpx; color: #f3d99a; font-size: 21rpx; letter-spacing: 1rpx; line-height: 1.4; }

.hero-copy { margin-top: 38rpx; }
.hero-kicker { display: block; color: #f3d99a; font-size: 22rpx; font-weight: 700; letter-spacing: 4rpx; }
.hero-title { display: block; max-width: 610rpx; margin-top: 12rpx; color: #fffdf8; font-size: 46rpx; font-weight: 900; letter-spacing: 1rpx; line-height: 1.28; }
.service-scope { display: flex; align-items: center; gap: 12rpx; margin-top: 20rpx; color: rgba(255, 249, 239, .88); font-size: 24rpx; line-height: 1.4; }
.scope-dot { width: 9rpx; height: 9rpx; border-radius: 50%; background: #f3d99a; box-shadow: 0 0 0 6rpx rgba(243, 217, 154, .12); }

.login-main {
  position: relative;
  z-index: 3;
  max-width: 720rpx;
  margin: -48rpx auto 0;
  padding: 0 28rpx calc(34rpx + env(safe-area-inset-bottom));
}

.login-card {
  padding: 34rpx;
  border: 1rpx solid #eadfcf;
  border-radius: 30rpx;
  background: #fffefa;
  box-shadow: 0 18rpx 48rpx rgba(91, 52, 38, .11);
}

.card-heading { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 14rpx; }
.card-title { color: #2a211b; font-size: 32rpx; font-weight: 900; line-height: 1.35; }
.demo-badge { display: inline-flex; align-items: center; min-height: 40rpx; padding: 5rpx 14rpx; border: 1rpx solid #c8953d; border-radius: 999rpx; background: #fff9eb; color: #80591e; font-size: 20rpx; font-weight: 700; line-height: 1.4; }

.login-field { margin-top: 30rpx; }
.field-label { display: block; margin-bottom: 12rpx; color: #554940; font-size: 24rpx; font-weight: 700; line-height: 1.4; }
.phone-input,
.code-input {
  width: 100%;
  height: 96rpx;
  padding: 0 26rpx;
  border: 2rpx solid #e2d8ca;
  border-radius: 20rpx;
  background: #fffdf8;
  color: #2a211b;
  font-size: 30rpx;
}
.phone-input:focus,
.code-input:focus { border-color: #a9151d; }

.code-field { margin-top: 24rpx; }
.code-row { display: flex; align-items: stretch; gap: 14rpx; }
.code-input { flex: 1; min-width: 0; }
.code-button { flex: 0 0 190rpx; width: 190rpx; height: 96rpx; margin: 0; padding: 0 12rpx; border: 1rpx solid #c8953d; border-radius: 20rpx; background: #fff8e8; color: #8b5d1c; font-size: 24rpx; font-weight: 800; line-height: 96rpx; }
.code-button[disabled] { opacity: 1; background: #f3eee5; color: #978b80; }
.code-button--pressed { background: #f7e7bf; }
.success-hint { display: block; margin-top: 10rpx; color: #32734a; font-size: 22rpx; line-height: 1.5; }

.error-panel { margin-top: 22rpx; }
.login-submit { height: 96rpx; min-height: 96rpx; margin: 30rpx 0 0; padding: 0 24rpx; border-radius: 22rpx; background: #a9151d; color: #fff; font-size: 29rpx; font-weight: 900; line-height: 96rpx; box-shadow: 0 14rpx 28rpx rgba(127, 16, 22, .2); }
.login-submit--pressed { background: #7f1016; transform: scale(.99); }
button.login-submit[disabled] { opacity: 1; background: #dbc9c6; color: #745f5c; box-shadow: none; }

.agreement-group { display: block; margin-top: 24rpx; }
.agreement-label { display: flex; align-items: flex-start; min-height: 48rpx; padding: 2rpx 0; }
.agreement-checkbox { flex: 0 0 46rpx; margin-top: 2rpx; transform: scale(.78); transform-origin: left top; }
.agreement-copy { flex: 1; min-width: 0; color: #756b62; font-size: 24rpx; line-height: 1.65; }
.agreement-link { color: #a9151d; font-weight: 700; }

.trust-copy { display: block; margin-top: 30rpx; color: #6d5f55; font-size: 23rpx; font-weight: 700; letter-spacing: 1rpx; line-height: 1.5; text-align: center; }
.platform-copy { display: block; max-width: 560rpx; margin: 15rpx auto 0; color: #988b81; font-size: 21rpx; line-height: 1.55; text-align: center; }

/* #ifdef MP-WEIXIN */
.login-hero { padding-top: calc(var(--status-bar-height) + 96rpx); }
/* #endif */

/* #ifdef H5 */
.login-hero { padding-top: calc(env(safe-area-inset-top) + 36rpx); }

@media (min-width: 720px) {
  .hero-inner, .login-main { max-width: 420px; }
  .login-hero { min-height: 430rpx; }
}

@media (min-height: 760px) and (max-width: 719px) {
  .login-hero { min-height: 700rpx; }
  .hero-inner { padding-top: 44rpx; }
  .hero-copy { margin-top: 54rpx; }
}

@media (max-height: 700px) {
  .hero-inner { padding-bottom: 70rpx; }
  .hero-copy { margin-top: 26rpx; }
  .login-main { margin-top: -42rpx; }
  .trust-copy { margin-top: 22rpx; }
}
/* #endif */
</style>
