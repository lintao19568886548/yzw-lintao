<script setup lang="ts">
import { computed } from 'vue'
import type { MatchResult } from '@/types/domain'
import { findDemoListing } from '@/fixtures/demo-listings'
import { mapMatchToCard } from '@/utils/recommendation'
import ListingVisual from './ListingVisual.vue'

const props = defineProps<{ result: MatchResult; selected?: boolean; busy?: boolean }>()
const emit = defineEmits<{ detail: [listingId: string]; toggleIntent: [listingId: string] }>()
const card = computed(() => mapMatchToCard(props.result))
const detail = computed(() => findDemoListing(props.result.listing.listing_id))
const features = computed(() => {
  const listing = props.result.listing
  return [
    listing.has_freight_elevator === null ? '货梯待核验' : listing.has_freight_elevator ? `货梯${listing.elevator_capacity_tons ? ` ${listing.elevator_capacity_tons}吨` : ''}` : '无货梯',
    listing.power_capacity_kva === null ? '用电待核验' : `${listing.power_capacity_kva}kVA`,
    listing.fire_rating ?? '消防待核验',
  ]
})
</script>

<template>
  <view class="listing-card card" :class="{ selected }">
    <ListingVisual compact :tone="detail?.image_tone ?? 'cinnabar'" :caption="detail?.image_caption ?? '脱敏示例房源'" @click="emit('detail', card.id)" />
    <view class="card-head">
      <view class="title-wrap">
        <view class="badge-row"><text class="tag red">{{ result.listing.verification_level.toUpperCase() }} 已核验</text><text class="tag">{{ result.listing.is_self_operated ? '宜租自营' : '合作业主' }}</text></view>
        <text class="listing-title">{{ card.title }}</text><text class="subtitle">{{ card.subtitle }}</text>
      </view>
      <view class="score"><text>{{ card.score }}</text><small>综合匹配</small></view>
    </view>
    <text class="rent">{{ card.rent }}</text>
    <view class="feature-row"><text v-for="feature in features" :key="feature">{{ feature }}</text></view>
    <view class="reason-list"><text v-for="reason in card.reasons" :key="reason">✓ {{ reason }}</text></view>
    <view v-if="card.blockedHard.length" class="hard-block"><text v-for="item in card.blockedHard" :key="item">× {{ item }}</text></view>
    <view v-if="card.unmetPreferences.length" class="preference-list"><text v-for="item in card.unmetPreferences" :key="item">偏好未满足：{{ item }}</text></view>
    <view v-if="card.warnings.length" class="warning-list"><text v-for="warning in card.warnings" :key="warning">待核验：{{ warning }}</text></view>
    <view class="button-row">
      <button class="ghost-button" @click="emit('detail', card.id)">查看详情</button>
      <button class="secondary-button" :class="{ selected }" :disabled="Boolean(busy) || card.submissionBlocked" @click="emit('toggleIntent', card.id)">{{ card.submissionBlocked ? '硬条件不符' : selected ? '✓ 已加入意向' : '+ 加入意向' }}</button>
    </view>
  </view>
</template>

<style scoped>
.listing-card { margin-bottom: 24rpx; transition: border-color .2s ease, transform .2s ease; }
.listing-card.selected { border: 2rpx solid #ae3035; box-shadow: 0 16rpx 42rpx rgba(143,23,32,.12); }
.card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 18rpx; margin-top: 22rpx; }
.title-wrap { min-width: 0; }.badge-row { display: flex; flex-wrap: wrap; gap: 8rpx; margin-bottom: 12rpx; }
.listing-title, .subtitle, .rent, .reason-list text, .warning-list text, .hard-block text, .preference-list text { display: block; }
.listing-title { color: #4b1418; font-size: 31rpx; font-weight: 900; }
.subtitle { margin-top: 7rpx; color: #765e54; font-size: 23rpx; }
.score { flex: 0 0 116rpx; text-align: center; color: #941d25; }
.score text { display: block; font-family: serif; font-size: 48rpx; font-weight: 900; line-height: 1; }
.score small { font-size: 18rpx; }
.rent { margin-top: 18rpx; color: #9a6517; font-size: 24rpx; font-weight: 800; }
.feature-row { display: flex; flex-wrap: wrap; gap: 10rpx; margin-top: 16rpx; }
.feature-row text { padding: 8rpx 12rpx; border-radius: 10rpx; background: #f5f0e6; color: #69564e; font-size: 20rpx; }
.reason-list { margin-top: 16rpx; color: #42633c; font-size: 22rpx; line-height: 1.75; }
.hard-block, .warning-list, .preference-list { margin-top: 14rpx; padding: 12rpx 16rpx; border-radius: 12rpx; font-size: 22rpx; line-height: 1.7; }
.hard-block { background: #fff0f0; color: #9b1c24; }.warning-list { background: #fff1e5; color: #8d4a27; }.preference-list { background: #fff8e8; color: #84601e; }
.button-row button { margin-top: 22rpx; font-size: 25rpx; }.secondary-button.selected { background: #a62028; color: #fff; }
</style>
