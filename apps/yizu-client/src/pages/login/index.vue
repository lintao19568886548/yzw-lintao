<script setup lang="ts">
import { ref } from 'vue'
import { onLoad } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { localDemoMode } from '@/config/runtime'
import { useAuthStore } from '@/stores/auth'
import { goHome } from '@/utils/navigation'
import { isValidPhone } from '@/utils/validation'

const auth = useAuthStore()
const phone = ref('13800138000')
const contactConfirmed = ref(false)
const loading = ref(false)
const errorMessage = ref('')

onLoad(() => {
  auth.hydrate()
  if (auth.is_authenticated) goHome()
})

async function login(): Promise<void> {
  errorMessage.value = ''
  if (!localDemoMode) {
    errorMessage.value = '当前构建未开启本地演示登录，请使用正式认证配置。'
    return
  }
  if (!isValidPhone(phone.value)) { errorMessage.value = '请输入有效的11位中国大陆手机号'; return }
  if (!contactConfirmed.value) { errorMessage.value = '请确认该手机号可用于顾问联系'; return }
  loading.value = true
  try {
    auth.setSession(await miniappApi.createDevSession({ phone: phone.value, contact_confirmed: true }))
    goHome()
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
        <view class="field"><text class="field-label">联系手机号</text><input v-model.trim="phone" class="input" type="number" maxlength="11" placeholder="请输入11位手机号" /></view>
        <view class="confirm-row" @click="contactConfirmed = !contactConfirmed">
          <checkbox :checked="contactConfirmed" color="#9b1d26" /><text>我确认该号码可接收招商顾问联系</text>
        </view>
        <view v-if="localDemoMode" class="notice">演示登录仅保存在本机，不发送短信，也不会通知真实顾问。</view>
        <view v-else class="notice">正式登录结构已预留：微信手机号、短信验证码和企业认证将在后续版本接入。</view>
        <view v-if="errorMessage" class="error-panel">{{ errorMessage }}</view>
        <button class="primary-button" :disabled="loading || !localDemoMode" @click="login">{{ loading ? '正在安全登录…' : '登录并开始 AI 找房' }}</button>
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
.login-card { padding: 34rpx; }.confirm-row { display: flex; align-items: center; gap: 12rpx; margin-top: 24rpx; color: #654d43; font-size: 24rpx; line-height: 1.5; }
.future-login { margin-top: 26rpx; padding: 26rpx; border: 1rpx dashed #d9c8a9; border-radius: 22rpx; background: rgba(255,253,247,.65); }
.future-title { display: block; margin-bottom: 10rpx; color: #6f4a2b; font-size: 24rpx; font-weight: 800; }
.future-login view { display: flex; justify-content: space-between; padding: 13rpx 0; color: #78665f; font-size: 22rpx; }.future-login view text:last-child { color: #a27a3f; }
.privacy-copy { display: block; margin: 26rpx 20rpx; text-align: center; color: #98877f; font-size: 20rpx; }
</style>
