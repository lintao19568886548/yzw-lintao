<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import DemandSummary from '@/components/DemandSummary.vue'
import StatePanel from '@/components/StatePanel.vue'
import { requireAuth } from '@/composables/useAuthGuard'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'
import { goHome } from '@/utils/navigation'

const auth = useAuthStore()
const demandStore = useDemandStore()
const history = useHistoryStore()

onShow(() => {
  auth.hydrate(); demandStore.hydrate(); history.hydrate(); requireAuth(auth)
})

function formatTime(epochSeconds: number): string {
  const date = new Date(epochSeconds * 1000)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')} ${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
}

function viewDetail(leadNumber: string): void {
  uni.navigateTo({ url: `/pages/success/index?lead=${encodeURIComponent(leadNumber)}` })
}

function repeat(index: number): void {
  const item = history.items[index]
  if (!item) return
  demandStore.prepareDemand(item.demand)
  goHome()
}
</script>

<template>
  <view class="page-shell content-width demands-page">
    <view class="page-head"><text class="page-kicker">MY DEMANDS</text><text class="page-title">我的找房需求</text><text class="page-desc">查看最近提交记录，或复用已有条件再次找房。</text></view>
    <view v-for="(item, index) in history.items" :key="item.lead_number" class="card demand-item">
      <view class="item-head"><view><text class="number">{{ item.demand_number }}</text><text class="created">{{ formatTime(item.created_at_epoch_seconds) }}</text></view><text class="status">待分配顾问</text></view>
      <DemandSummary :demand="item.demand" />
      <view class="listing-names"><text class="field-label">意向房源</text><text v-for="listing in item.listings" :key="listing.listing_id">· {{ listing.listing_name }}</text></view>
      <view class="button-row"><button class="ghost-button" @click="viewDetail(item.lead_number)">查看详情</button><button class="secondary-button" @click="repeat(index)">再次找房</button></view>
    </view>
    <StatePanel v-if="!history.items.length" icon="需" title="还没有已提交需求" description="从首页描述厂房、仓库或写字楼需求，AI 会为你整理条件并推荐房源。"><button class="primary-button" @click="goHome">开始 AI 找房</button></StatePanel>
  </view>
</template>

<style scoped>
.demand-item { margin-bottom: 22rpx; }.item-head { display: flex; justify-content: space-between; margin-bottom: 20rpx; }.number,.created { display: block; }.number { color: #5b1e21; font-size: 28rpx; font-weight: 900; }.created { margin-top: 6rpx; color: #927f77; font-size: 19rpx; }.status { align-self: flex-start; padding: 8rpx 14rpx; border-radius: 999rpx; background: #fff0d0; color: #98651c; font-size: 20rpx; font-weight: 800; }
.listing-names { margin-top: 20rpx; padding-top: 18rpx; border-top: 1rpx solid #eee2cf; }.listing-names > text:not(.field-label) { display: block; margin-top: 8rpx; color: #6e5850; font-size: 22rpx; }.button-row button { margin-top: 20rpx; font-size: 24rpx; }
</style>
