<script setup lang="ts">
import { computed, ref } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import DemandSummary from '@/components/DemandSummary.vue'
import ListingVisual from '@/components/ListingVisual.vue'
import { miniappApi } from '@/api/miniapp'
import { isSessionError, mapApiError } from '@/api/client'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState } from '@/composables/useDemandForm'
import { findDemoListing } from '@/fixtures/demo-listings'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'

const auth = useAuthStore()
const store = useDemandStore()
const history = useHistoryStore()
const contactConfirmed = ref(false)
const privacyAgreed = ref(false)
const serviceAgreed = ref(false)
const errorMessage = ref('')
const selectedResults = computed(() => (store.match_response?.matches ?? []).filter((match) => store.selected_listing_ids.includes(match.listing.listing_id)))

onShow(() => {
  restoreFlowState(auth, store, () => undefined)
  history.hydrate()
  if (!requireAuth(auth)) return
  if (!store.match_response || !store.selected_listing_ids.length) uni.navigateBack()
})

async function submit(): Promise<void> {
  if (!requireAuth(auth)) return
  if (!contactConfirmed.value) { errorMessage.value = '请确认当前手机号可用于顾问联系'; return }
  if (!privacyAgreed.value || !serviceAgreed.value) { errorMessage.value = '请阅读并同意隐私政策、用户协议与服务说明'; return }
  if (!selectedResults.value.length) { errorMessage.value = '请至少选择一套有效意向房源'; return }
  if (store.lead) { uni.redirectTo({ url: `/pages/success/index?lead=${encodeURIComponent(store.lead.lead_number)}` }); return }
  if (!store.beginLeadSubmission()) return
  errorMessage.value = ''
  try {
    const lead = await miniappApi.submitLead({
      session_token: auth.session_token,
      submission: {
        demand: store.demand,
        recommended_listing_ids: [...store.selected_listing_ids],
        source_channel: 'miniapp_ai_demand',
        idempotency_key: store.idempotency_key,
      },
    })
    store.setLead(lead)
    history.recordSubmission(lead, auth.masked_phone, store.match_response)
    uni.redirectTo({ url: `/pages/success/index?lead=${encodeURIComponent(lead.lead_number)}` })
  } catch (error) {
    if (isSessionError(error)) { auth.clear(); requireAuth(auth); return }
    errorMessage.value = mapApiError(error)
  } finally {
    store.finishLeadSubmission()
  }
}
</script>

<template>
  <view class="page-shell content-width lead-page">
    <view class="page-head"><text class="page-kicker">最后一步</text><text class="page-title">确认找房需求与联系方式</text><text class="page-desc">提交后由招商顾问核实房源现状、价格与带看安排。服务端会重新校验硬条件。</text></view>
    <view class="card"><text class="section-title">需求摘要</text><DemandSummary :demand="store.demand" /></view>
    <view class="card"><view class="section-row"><text class="section-title">意向房源</text><text class="tag red">已选 {{ selectedResults.length }} 套</text></view>
      <view v-for="result in selectedResults" :key="result.listing.listing_id" class="intent-item">
        <ListingVisual compact :tone="findDemoListing(result.listing.listing_id)?.image_tone ?? 'cinnabar'" :caption="result.listing.listing_name" />
        <view><strong>{{ result.listing.listing_name }}</strong><text>{{ result.listing.town }} · {{ result.listing.available_area_sqm }}㎡ · 匹配 {{ result.overall_score.toFixed(1) }}分</text></view>
      </view>
    </view>
    <view class="card contact-card">
      <text class="section-title">联系方式</text>
      <view class="phone-row"><view><text>当前登录手机号</text><strong>{{ auth.masked_phone }}</strong></view><text class="tag green">已脱敏</text></view>
      <label class="check-row"><checkbox :checked="contactConfirmed" color="#9b1d26" @click="contactConfirmed = !contactConfirmed" /><text>我确认该手机号可接听招商顾问来电</text></label>
    </view>
    <view class="card service-card"><text class="section-title">服务说明</text>
      <view class="service-item"><text>01</text><view><strong>免费需求对接</strong><small>当前演示不产生付款或服务费账单</small></view></view>
      <view class="service-item"><text>02</text><view><strong>房源二次核验</strong><small>价格、空置和硬条件由服务端及顾问复核</small></view></view>
      <view class="service-item"><text>03</text><view><strong>15分钟响应承诺</strong><small>仅指工作时间目标首次联系时效</small></view></view>
    </view>
    <view class="agreement-box">
      <label class="check-row"><checkbox :checked="privacyAgreed" color="#9b1d26" @click="privacyAgreed = !privacyAgreed" /><text>我已阅读并同意《隐私政策》与《用户协议》</text></label>
      <label class="check-row"><checkbox :checked="serviceAgreed" color="#9b1d26" @click="serviceAgreed = !serviceAgreed" /><text>我同意宜租网就本次找房需求提供顾问服务</text></label>
    </view>
    <view v-if="errorMessage" class="error-panel">{{ errorMessage }}，可检查后重试。</view>
    <button class="primary-button" :disabled="store.submitting" @click="submit">{{ store.submitting ? '正在安全提交，请勿重复点击…' : '提交找房需求' }}</button>
    <text class="idempotency">本次提交已启用幂等保护，不会因重复点击生成重复线索。</text>
  </view>
</template>

<style scoped>
.lead-page { padding-bottom: 70rpx; }.section-row,.phone-row { display: flex; align-items: center; justify-content: space-between; }.intent-item { display: grid; grid-template-columns: 210rpx 1fr; gap: 18rpx; align-items: center; padding: 18rpx 0; border-bottom: 1rpx solid #eee2cf; }.intent-item:last-child { border-bottom: 0; }.intent-item strong,.intent-item text { display: block; }.intent-item strong { color: #512f2a; font-size: 24rpx; }.intent-item text { margin-top: 8rpx; color: #89766f; font-size: 20rpx; line-height: 1.5; }
.phone-row text,.phone-row strong { display: block; }.phone-row text { color: #8a7770; font-size: 21rpx; }.phone-row strong { margin-top: 7rpx; color: #551e21; font-size: 32rpx; }.check-row { display: flex; align-items: flex-start; gap: 10rpx; margin-top: 22rpx; color: #66514a; font-size: 23rpx; line-height: 1.55; }
.service-item { display: flex; gap: 16rpx; margin-top: 18rpx; }.service-item > text { display: grid; flex: 0 0 48rpx; width: 48rpx; height: 48rpx; place-items: center; border-radius: 14rpx; background: #f5e7cc; color: #97651e; font-size: 19rpx; font-weight: 900; }.service-item strong,.service-item small { display: block; }.service-item strong { color: #55352f; font-size: 24rpx; }.service-item small { margin-top: 5rpx; color: #8b7770; font-size: 20rpx; }
.agreement-box { margin-top: 24rpx; padding: 22rpx; border: 1rpx dashed #d9c6a7; border-radius: 20rpx; background: #fffaf1; }.agreement-box .check-row:first-child { margin-top: 0; }.idempotency { display: block; margin: 16rpx 10rpx; text-align: center; color: #8c7b74; font-size: 19rpx; }
</style>
