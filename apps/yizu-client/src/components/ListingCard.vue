<script setup lang="ts">
import { computed } from 'vue'
import type { MatchResult } from '@/types/domain'
import { mapMatchToCard } from '@/utils/recommendation'
import ScoreBar from './ScoreBar.vue'

const props = defineProps<{ result: MatchResult; busy?: boolean }>()
const emit = defineEmits<{ contact: [listingId: string] }>()
const card = computed(() => mapMatchToCard(props.result))

const dimensionLabels: Record<string, string> = {
  location: '位置', space: '空间', cost: '成本', production: '生产',
  logistics: '物流', compliance: '合规', move_in: '入驻',
}
</script>

<template>
  <view class="listing-card card">
    <view class="card-head">
      <view>
        <text class="listing-title">{{ card.title }}</text>
        <text class="subtitle">{{ card.subtitle }}</text>
      </view>
      <view class="score"><text>{{ card.score }}</text><small>综合分</small></view>
    </view>
    <text class="rent">{{ card.rent }}</text>
    <view class="meta">
      <text>{{ card.verification }}核验</text><text>{{ card.source }}</text><text>更新 {{ result.listing.updated_at }}</text>
    </view>
    <view class="reason-list"><text v-for="reason in card.reasons" :key="reason">✓ {{ reason }}</text></view>
    <view v-if="card.warnings.length" class="warning-list"><text v-for="warning in card.warnings" :key="warning">· {{ warning }}</text></view>
    <view class="divider" />
    <ScoreBar v-for="item in result.dimension_scores" :key="item.dimension" :label="dimensionLabels[item.dimension] ?? item.dimension" :score="item.score" :reason="item.reason" />
    <button class="primary-button" :disabled="Boolean(busy)" @click="emit('contact', card.id)">联系顾问</button>
  </view>
</template>

<style scoped>
.listing-card { margin-bottom: 26rpx; }
.card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 20rpx; }
.listing-title, .subtitle, .rent, .reason-list text, .warning-list text { display: block; }
.listing-title { color: #4b1418; font-size: 32rpx; font-weight: 900; }
.subtitle { margin-top: 8rpx; color: #765e54; font-size: 24rpx; }
.score { flex: 0 0 116rpx; text-align: center; color: #941d25; }
.score text { display: block; font-family: serif; font-size: 50rpx; font-weight: 900; }
.score small { font-size: 20rpx; }
.rent { margin-top: 20rpx; color: #9a6517; font-size: 25rpx; font-weight: 800; }
.meta { display: flex; flex-wrap: wrap; gap: 12rpx; margin-top: 18rpx; }
.meta text { padding: 8rpx 14rpx; border-radius: 10rpx; background: #f4ead5; color: #705b4f; font-size: 20rpx; }
.reason-list, .warning-list { margin-top: 18rpx; font-size: 23rpx; line-height: 1.8; }
.reason-list { color: #42633c; }
.warning-list { padding: 12rpx 18rpx; border-radius: 12rpx; background: #fff1e5; color: #8d4a27; }
</style>
