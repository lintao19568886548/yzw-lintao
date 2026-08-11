<script setup lang="ts">
import { computed, ref } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import DemandSummary from '@/components/DemandSummary.vue'
import ListingCard from '@/components/ListingCard.vue'
import StatePanel from '@/components/StatePanel.vue'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState } from '@/composables/useDemandForm'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'

const auth = useAuthStore()
const store = useDemandStore()
const message = ref('')
const selectedCount = computed(() => store.selected_listing_ids.length)

onShow(() => {
  restoreFlowState(auth, store, () => undefined)
  if (!requireAuth(auth)) return
  if (!store.match_response) uni.switchTab({ url: '/pages/home/index' })
})

function viewDetail(listingId: string): void {
  uni.navigateTo({ url: `/pages/listing-detail/index?id=${encodeURIComponent(listingId)}` })
}

function toggleIntent(listingId: string): void {
  message.value = ''
  if (!store.toggleListing(listingId)) {
    message.value = '该房源存在未满足或无法核验的硬条件，不能加入意向。'
    return
  }
  uni.showToast({ title: store.selected_listing_ids.includes(listingId) ? '已加入意向' : '已取消意向', icon: 'none' })
}

function continueToLead(): void {
  if (!selectedCount.value) { message.value = '请先选择至少一套意向房源'; return }
  uni.navigateTo({ url: '/pages/lead-confirm/index' })
}

function adjustDemand(): void { uni.navigateBack() }
</script>

<template>
  <view class="page-shell content-width results-page">
    <view class="page-head">
      <DemoBadge />
      <text class="page-kicker">七维智能匹配</text>
      <text class="page-title">为你推荐 {{ store.match_response?.matches.length ?? 0 }} 套房源</text>
      <text class="page-desc">位置、空间、成本、生产、物流、合规与入驻综合评分；核验等级与更新时间用于同分修正。</text>
    </view>
    <view class="card demand-card"><text class="field-label">本次找房需求</text><DemandSummary :demand="store.demand" /></view>
    <view v-if="store.match_response?.used_area_relaxation" class="notice">严格面积无结果，已按规则放宽面积范围 ±20%，没有自动扩大镇街。</view>
    <view v-if="store.match_response?.next_step_suggestion" class="notice">{{ store.match_response.next_step_suggestion }}</view>
    <view v-if="message" class="error-panel">{{ message }}</view>
    <view class="result-list">
      <ListingCard v-for="result in store.match_response?.matches ?? []" :key="result.listing.listing_id" :result="result" :selected="store.selected_listing_ids.includes(result.listing.listing_id)" :busy="store.submitting" @detail="viewDetail" @toggle-intent="toggleIntent" />
    </view>
    <StatePanel v-if="(store.match_response?.matches.length ?? 0) === 0" icon="寻" title="暂时没有合适的已核验房源" description="我们不会把 L0/L1、暂停出租或硬条件无法核验的房源伪装成推荐。你可以调整镇街、面积或条件优先级。">
      <button class="secondary-button" @click="adjustDemand">返回调整需求</button>
    </StatePanel>
    <view v-if="(store.match_response?.matches.length ?? 0) > 0" class="sticky-action">
      <view class="selection"><strong>已选 {{ selectedCount }} 套</strong><text>最多选择 5 套意向房源</text></view>
      <button class="primary-button" @click="continueToLead">确认意向并提交</button>
    </view>
  </view>
</template>

<style scoped>
.results-page { padding-bottom: 70rpx; }.page-head .demo-badge { margin-bottom: 20rpx; }.demand-card { margin-bottom: 22rpx; }.result-list { margin-top: 26rpx; }
.selection { flex: 0 0 200rpx; }.selection strong,.selection text { display: block; }.selection strong { color: #68151b; font-size: 26rpx; }.selection text { margin-top: 5rpx; color: #8b7770; font-size: 18rpx; }
</style>
