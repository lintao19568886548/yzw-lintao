<script setup lang="ts">
import { ref } from 'vue'
import { onLoad } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { localDemoMode } from '@/config/runtime'
import { useAuthStore } from '@/stores/auth'
import { isValidPhone } from '@/utils/validation'

const auth = useAuthStore()
const phone = ref('')
const contactConfirmed = ref(false)
const loading = ref(false)
const errorMessage = ref('')

onLoad(() => {
  auth.hydrate()
  if (auth.is_authenticated) uni.reLaunch({ url: '/pages/home/index' })
})

async function login(): Promise<void> {
  errorMessage.value = ''
  if (!localDemoMode) {
    errorMessage.value = '当前构建未启用本地演示登录，请配置正式登录方式。'
    return
  }
  if (!isValidPhone(phone.value)) {
    errorMessage.value = '请输入有效的11位中国大陆手机号'
    return
  }
  if (!contactConfirmed.value) {
    errorMessage.value = '请确认该手机号可用于顾问联系'
    return
  }
  loading.value = true
  try {
    const session = await miniappApi.createDevSession({ phone: phone.value, contact_confirmed: true })
    auth.setSession(session)
    uni.reLaunch({ url: '/pages/home/index' })
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
      <view class="card login-card">
        <DemoBadge />
        <text class="section-title">企业找房登录</text>
        <text class="section-subtitle">首轮只提供本地开发模拟登录，不发送真实短信，也不会通知真实顾问。</text>
        <view class="field">
          <text class="field-label">联系手机号</text>
          <input v-model.trim="phone" class="input" type="number" maxlength="11" placeholder="请输入11位手机号" />
        </view>
        <view class="confirm-row" @click="contactConfirmed = !contactConfirmed">
          <checkbox :checked="contactConfirmed" color="#8f1720" />
          <text>我确认该手机号可用于本次找房顾问联系</text>
        </view>
        <view v-if="!localDemoMode" class="notice">生产构建不会自动获得模拟身份。当前未接入微信登录、短信验证码和企业认证。</view>
        <text v-if="errorMessage" class="error">{{ errorMessage }}</text>
        <button class="primary-button" :disabled="loading || !localDemoMode" @click="login">{{ loading ? '正在创建演示会话…' : '进入 AI 找房' }}</button>
      </view>
    </view>
  </view>
</template>

<style scoped>
.login-page { min-height: 100vh; background: radial-gradient(circle at 80% 10%, #f3dfae 0, transparent 40%), #f7f2e8; }
.login-card { margin-top: 40rpx; }
.section-title { margin-top: 28rpx; }
.confirm-row { display: flex; align-items: center; gap: 12rpx; margin-top: 24rpx; color: #654d43; font-size: 24rpx; line-height: 1.5; }
</style>
