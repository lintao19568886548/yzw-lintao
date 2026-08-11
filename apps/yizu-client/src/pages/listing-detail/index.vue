<script setup lang="ts">
import { computed, ref } from 'vue'
import { onLoad, onShow } from '@dcloudio/uni-app'
import ListingVisual from '@/components/ListingVisual.vue'
import ScoreBar from '@/components/ScoreBar.vue'
import StatePanel from '@/components/StatePanel.vue'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState } from '@/composables/useDemandForm'
import { findDemoListing } from '@/fixtures/demo-listings'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import type { ListingDetail } from '@/types/domain'
import { formatCentsAsYuan } from '@/utils/money'

const auth = useAuthStore()
const store = useDemandStore()
const listingId = ref('')
const message = ref('')

onLoad((query) => { listingId.value = typeof query?.id === 'string' ? decodeURIComponent(query.id) : '' })
onShow(() => {
  restoreFlowState(auth, store, () => undefined)
  requireAuth(auth)
})

const result = computed(() => store.match_response?.matches.find((item) => item.listing.listing_id === listingId.value) ?? null)
const listing = computed<ListingDetail | null>(() => {
  const demo = findDemoListing(listingId.value)
  if (demo) return demo
  const summary = result.value?.listing
  if (!summary) return null
  return { ...summary, approximate_location: `${summary.town}产业片区`, building_height_m: null, image_caption: '脱敏房源示意', image_tone: 'cinnabar' }
})
const selected = computed(() => store.selected_listing_ids.includes(listingId.value))
const blocked = computed(() => Boolean(result.value?.unmet_hard_constraints.length || result.value?.unverified_hard_constraints.length))
const dimensionLabels: Record<string, string> = { location: '位置', space: '空间', cost: '成本', production: '生产', logistics: '物流', compliance: '合规', move_in: '入驻' }

function displayBoolean(value: boolean | null, yes: string, no: string): string {
  return value === null ? '待核验' : value ? yes : no
}

function toggleIntent(): void {
  if (!store.toggleListing(listingId.value)) { message.value = '该房源存在硬条件阻断，暂不能加入意向。'; return }
  message.value = ''
  uni.showToast({ title: selected.value ? '已加入意向' : '已取消意向', icon: 'none' })
}

function consult(): void {
  uni.showModal({ title: '电话咨询', content: '当前为脱敏演示，不展示或拨打真实业主电话。提交找房需求后，招商顾问将在工作时间15分钟内回电。', showCancel: false })
}

function backToResults(): void { uni.navigateBack() }
</script>

<template>
  <view class="page-shell content-width detail-page">
    <template v-if="listing && result">
      <ListingVisual :tone="listing.image_tone" :caption="listing.image_caption" />
      <view class="detail-head">
        <view class="badge-row"><text class="tag red">{{ listing.verification_level.toUpperCase() }} 已核验</text><text class="tag gold">{{ listing.is_self_operated ? '宜租网自营' : '合作业主' }}</text></view>
        <text class="page-title">{{ listing.listing_name }}</text><text class="page-desc">{{ listing.town }} · {{ listing.approximate_location }}（不展示精确门牌）</text>
        <view class="score-line"><strong>{{ result.overall_score.toFixed(1) }}</strong><view><text>综合匹配分</text><small>{{ result.recommendation_reasons[0] ?? '综合条件匹配' }}</small></view></view>
      </view>
      <view class="card price-card"><text>参考月租</text><strong>¥{{ formatCentsAsYuan(listing.monthly_rent_cents) }}</strong><small>¥{{ formatCentsAsYuan(listing.rent_cents_per_sqm_month) }}/㎡/月 · 仅供演示</small></view>
      <view class="card"><text class="section-title">空间参数</text><view class="spec-grid">
        <view><text>空间类型</text><strong>{{ listing.space_type === 'factory' ? '厂房' : listing.space_type === 'warehouse' ? '仓库' : '写字楼' }}</strong></view>
        <view><text>可租面积</text><strong>{{ listing.available_area_sqm }}㎡</strong></view>
        <view><text>楼层</text><strong>{{ listing.floor_label ?? '待核验' }}</strong></view>
        <view><text>层高</text><strong>{{ listing.building_height_m ? `${listing.building_height_m}米` : '待核验' }}</strong></view>
        <view><text>货梯</text><strong>{{ displayBoolean(listing.has_freight_elevator, '有', '无') }}</strong></view>
        <view><text>电梯吨位</text><strong>{{ listing.elevator_capacity_tons ? `${listing.elevator_capacity_tons}吨` : '待核验/无' }}</strong></view>
        <view><text>用电容量</text><strong>{{ listing.power_capacity_kva ? `${listing.power_capacity_kva}kVA` : '待核验' }}</strong></view>
        <view><text>可入驻时间</text><strong>{{ listing.available_from }}</strong></view>
      </view></view>
      <view class="card"><text class="section-title">生产、物流与合规</text><view class="spec-list">
        <view><text>消防等级</text><strong>{{ listing.fire_rating ?? '待核验' }}</strong></view>
        <view><text>货车通行</text><strong>{{ displayBoolean(listing.truck_access, '支持', '不支持') }}</strong></view>
        <view><text>装卸条件</text><strong>{{ displayBoolean(listing.loading_dock, '具备装卸区/月台', '无专用月台') }}</strong></view>
        <view><text>分租</text><strong>{{ displayBoolean(listing.allows_sublease, '可协商分租', '整租') }}</strong></view>
        <view><text>更新时间</text><strong>{{ listing.updated_at }}</strong></view>
      </view></view>
      <view class="card"><text class="section-title">为什么推荐</text><text v-for="reason in result.recommendation_reasons" :key="reason" class="reason">✓ {{ reason }}</text><view class="divider" /><ScoreBar v-for="item in result.dimension_scores" :key="item.dimension" :label="dimensionLabels[item.dimension] ?? item.dimension" :score="item.score" :reason="item.reason" /></view>
      <view v-if="listing.data_gaps.length" class="notice">待核验项：{{ listing.data_gaps.join('、') }}</view>
      <view v-if="blocked" class="error-panel">该房源存在未满足或无法核验的硬条件，只可查看，不能加入可提交意向。</view>
      <view v-if="message" class="error-panel">{{ message }}</view>
      <view class="button-row"><button class="ghost-button" @click="backToResults">返回推荐</button><button class="secondary-button" @click="consult">电话咨询</button></view>
      <button class="primary-button" :disabled="blocked" @click="toggleIntent">{{ selected ? '✓ 已加入意向 · 点击取消' : '+ 加入意向房源' }}</button>
    </template>
    <StatePanel v-else icon="房" title="房源信息暂不可用" description="推荐结果可能已刷新，请返回重新匹配。"><button class="secondary-button" @click="backToResults">返回推荐结果</button></StatePanel>
  </view>
</template>

<style scoped>
.detail-page { padding-top: 24rpx; }.detail-head { padding: 26rpx 6rpx; }.badge-row { display: flex; gap: 10rpx; margin-bottom: 16rpx; }.score-line { display: flex; align-items: center; gap: 18rpx; margin-top: 22rpx; }.score-line > strong { color: #a31d26; font-family: serif; font-size: 58rpx; }.score-line text,.score-line small { display: block; }.score-line text { color: #5c3932; font-size: 24rpx; font-weight: 900; }.score-line small { margin-top: 5rpx; color: #8b7770; font-size: 20rpx; }
.price-card { display: flex; align-items: baseline; gap: 14rpx; }.price-card text { color: #76645c; font-size: 22rpx; }.price-card strong { color: #9c5f13; font-size: 36rpx; }.price-card small { color: #8e7a70; font-size: 19rpx; }
.spec-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 18rpx; }.spec-grid view { padding: 18rpx; border-radius: 15rpx; background: #faf6ee; }.spec-grid text,.spec-grid strong,.spec-list text,.spec-list strong { display: block; }.spec-grid text,.spec-list text { color: #8b7770; font-size: 20rpx; }.spec-grid strong,.spec-list strong { margin-top: 7rpx; color: #50322c; font-size: 24rpx; }
.spec-list view { display: flex; align-items: center; justify-content: space-between; padding: 17rpx 0; border-bottom: 1rpx solid #eee3d0; }.spec-list view:last-child { border-bottom: 0; }.spec-list strong { margin: 0; text-align: right; }.reason { display: block; margin-top: 10rpx; color: #3f6d49; font-size: 23rpx; line-height: 1.6; }
</style>
