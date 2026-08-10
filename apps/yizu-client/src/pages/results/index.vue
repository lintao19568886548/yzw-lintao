<script setup lang="ts">
import { ref } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import ListingCard from '@/components/ListingCard.vue'
import { miniappApi } from '@/api/miniapp'
import { mapApiError } from '@/api/client'
import { requireAuth } from '@/composables/useAuthGuard'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'

const auth = useAuthStore()
const store = useDemandStore()
const errorMessage = ref('')

onShow(() => {
  auth.hydrate()
  store.hydrate()
  if (!requireAuth(auth)) return
  if (!store.match_response) uni.reLaunch({ url: '/pages/home/index' })
})

async function contact(listingId: string): Promise<void> {
  if (!requireAuth(auth) || !store.beginLeadSubmission()) return
  errorMessage.value = ''
  try {
    const lead = await miniappApi.submitLead({
      session_token: auth.session_token,
      submission: {
        demand: store.demand,
        recommended_listing_ids: [listingId],
        source_channel: 'miniapp_ai_demand',
        idempotency_key: store.idempotency_key,
      },
    })
    store.setLead(lead)
    uni.navigateTo({ url: '/pages/success/index' })
  } catch (error) {
    errorMessage.value = mapApiError(error)
  } finally {
    store.finishLeadSubmission()
  }
}

function goBack(): void {
  uni.navigateBack()
}
</script>

<template>
  <view class="page-shell content-width">
    <view class="result-head">
      <DemoBadge />
      <text class="section-title">为你找到 {{ store.match_response?.matches.length ?? 0 }} 套推荐</text>
      <text class="section-subtitle">综合分由 Rust BFF 按七维规则计算；核验等级和更新时间只用于同分排序。</text>
      <view v-if="store.match_response?.used_area_relaxation" class="notice">严格面积无结果，已按规则放宽面积范围 ±20%，未扩展镇街。</view>
      <view v-if="store.match_response?.next_step_suggestion" class="notice">{{ store.match_response.next_step_suggestion }}</view>
      <text v-if="errorMessage" class="error">{{ errorMessage }}</text>
    </view>
    <ListingCard v-for="result in store.match_response?.matches ?? []" :key="result.listing.listing_id" :result="result" :busy="store.submitting" @contact="contact" />
    <view v-if="(store.match_response?.matches.length ?? 0) === 0" class="card empty-state">
      <text class="section-title">暂时没有合适的已核验房源</text>
      <text class="section-subtitle">我们不会把 L0/L1 或暂停出租房源放进 AI 推荐，也不会未经确认自动扩大到相邻镇街。</text>
      <button class="secondary-button" @click="goBack">返回调整需求</button>
    </view>
  </view>
</template>

<style scoped>
.result-head { margin-bottom: 26rpx; }
.result-head .section-title { margin-top: 24rpx; }
.empty-state { text-align: center; }
</style>
