<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import { requireAuth } from '@/composables/useAuthGuard'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'

const auth = useAuthStore()
const store = useDemandStore()

onShow(() => {
  auth.hydrate()
  store.hydrate()
  if (!requireAuth(auth)) return
  if (!store.lead) uni.reLaunch({ url: '/pages/home/index' })
})

function startAgain(): void {
  store.resetFlow()
  uni.reLaunch({ url: '/pages/home/index' })
}
</script>

<template>
  <view class="page-shell content-width success-page">
    <view class="seal-check">✓</view>
    <DemoBadge />
    <text class="success-title">本地演示线索已生成</text>
    <text class="section-subtitle">工作时间内的目标首次联系 SLA 为 15 分钟，但本地模式没有通知任何真实招商顾问。</text>
    <view class="card numbers">
      <view><text>需求编号</text><strong>{{ store.lead?.demand_number }}</strong></view>
      <view><text>线索编号</text><strong>{{ store.lead?.lead_number }}</strong></view>
      <view><text>当前状态</text><strong>待分配（演示）</strong></view>
      <view><text>联系号码</text><strong>{{ auth.masked_phone }}</strong></view>
    </view>
    <view class="notice">线索只保存在 Rust 进程的临时内存中，服务重启后可能丢失；未发送电话、短信、企业微信或外部通知。</view>
    <button class="primary-button" @click="startAgain">继续找房</button>
  </view>
</template>

<style scoped>
.success-page { padding-top: 90rpx; text-align: center; }
.seal-check { display: grid; width: 132rpx; height: 132rpx; margin: 0 auto 28rpx; place-items: center; border: 5rpx double #c3943d; border-radius: 50%; background: #8f1720; color: #fff2c8; font-size: 70rpx; font-weight: 900; }
.success-title { display: block; margin: 28rpx 0 16rpx; color: #711018; font-family: serif; font-size: 44rpx; font-weight: 900; }
.numbers { margin-top: 34rpx; text-align: left; }
.numbers view { display: flex; justify-content: space-between; padding: 18rpx 0; border-bottom: 1rpx solid #ecdfc7; color: #725d53; font-size: 25rpx; }
.numbers view:last-child { border-bottom: 0; }
.numbers strong { color: #4a1718; }
</style>
