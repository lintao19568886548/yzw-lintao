<script setup lang="ts">
import { computed, ref } from 'vue'
import { onLoad, onShow } from '@dcloudio/uni-app'
import DemoBadge from '@/components/DemoBadge.vue'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState } from '@/composables/useDemandForm'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'
import { goHome, goMyDemands } from '@/utils/navigation'

const auth = useAuthStore()
const store = useDemandStore()
const history = useHistoryStore()
const leadNumber = ref('')

onLoad((query) => { leadNumber.value = typeof query?.lead === 'string' ? decodeURIComponent(query.lead) : '' })
onShow(() => {
  restoreFlowState(auth, store, () => undefined)
  history.hydrate()
  if (!requireAuth(auth)) return
  if (!store.lead && !history.items.some((item) => item.lead_number === leadNumber.value)) goHome()
})

const historyItem = computed(() => history.items.find((item) => item.lead_number === leadNumber.value) ?? history.items[0] ?? null)
const demandNumber = computed(() => store.lead?.lead_number === leadNumber.value ? store.lead.demand_number : historyItem.value?.demand_number ?? '')
const currentLeadNumber = computed(() => leadNumber.value || store.lead?.lead_number || historyItem.value?.lead_number || '')
const listings = computed(() => historyItem.value?.listings ?? (store.match_response?.matches ?? []).filter((match) => store.selected_listing_ids.includes(match.listing.listing_id)).map((match) => match.listing))
const maskedPhone = computed(() => historyItem.value?.masked_phone || auth.masked_phone)

function startAgain(): void { store.resetFlow(); goHome() }
</script>

<template>
  <view class="page-shell content-width success-page">
    <view class="success-mark"><text>✓</text></view>
    <DemoBadge />
    <text class="success-title">找房需求提交成功</text>
    <text class="success-promise">招商顾问将在工作时间 <strong>15分钟内</strong> 联系您</text>
    <text class="page-desc">顾问将再次核实意向房源的空置、价格与硬条件，不会直接把你的号码展示给业主。</text>
    <view class="card numbers">
      <view><text>需求编号</text><strong>{{ demandNumber }}</strong></view>
      <view><text>线索编号</text><strong>{{ currentLeadNumber }}</strong></view>
      <view><text>当前状态</text><strong class="status">待分配顾问</strong></view>
      <view><text>联系号码</text><strong>{{ maskedPhone }}</strong></view>
    </view>
    <view class="card"><text class="section-title">已提交意向房源</text><view v-for="listing in listings" :key="listing.listing_id" class="listing-row"><view><strong>{{ listing.listing_name }}</strong><text>{{ listing.town }} · {{ listing.available_area_sqm }}㎡</text></view><text class="tag red">已提交</text></view></view>
    <view class="notice">当前为本地演示：线索保存在本机与临时内存中，没有发送真实电话、短信、企业微信或外部通知。</view>
    <button class="primary-button" @click="goMyDemands">查看我的需求</button>
    <button class="secondary-button" @click="startAgain">返回首页 · 再次找房</button>
  </view>
</template>

<style scoped>
.success-page { padding-top: 62rpx; text-align: center; }.success-mark { position: relative; display: grid; width: 138rpx; height: 138rpx; margin: 0 auto 26rpx; place-items: center; border: 5rpx double #d6a748; border-radius: 50%; background: linear-gradient(145deg,#aa2830,#711018); color: #fff3c8; box-shadow: 0 17rpx 36rpx rgba(120,18,25,.2); }.success-mark::after { position: absolute; inset: -16rpx; content: ''; border: 1rpx solid rgba(198,148,60,.3); border-radius: 50%; }.success-mark text { font-size: 70rpx; font-weight: 900; }
.success-title { display: block; margin: 24rpx 0 14rpx; color: #711018; font-family: serif; font-size: 44rpx; font-weight: 900; }.success-promise { display: block; color: #5c4038; font-size: 27rpx; }.success-promise strong { color: #a11f27; font-size: 34rpx; }
.numbers { margin-top: 32rpx; text-align: left; }.numbers view { display: flex; justify-content: space-between; padding: 18rpx 0; border-bottom: 1rpx solid #ecdfc7; color: #725d53; font-size: 24rpx; }.numbers view:last-child { border-bottom: 0; }.numbers strong { color: #4a1718; }.numbers .status { color: #a5681e; }
.listing-row { display: flex; align-items: center; justify-content: space-between; padding: 17rpx 0; border-bottom: 1rpx solid #eee2cf; text-align: left; }.listing-row:last-child { border-bottom: 0; }.listing-row strong,.listing-row text { display: block; }.listing-row strong { color: #52332d; font-size: 24rpx; }.listing-row view text { margin-top: 5rpx; color: #8a7770; font-size: 20rpx; }
</style>
