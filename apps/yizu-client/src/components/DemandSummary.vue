<script setup lang="ts">
import { computed } from 'vue'
import type { DemandDraft, SpaceType } from '@/types/domain'
import { formatCentsAsYuan } from '@/utils/money'

const props = defineProps<{ demand: DemandDraft; compact?: boolean }>()
const spaceLabels: Record<SpaceType, string> = { factory: '厂房', warehouse: '仓库', office: '写字楼' }
const items = computed(() => {
  const c = props.demand.constraints
  return [
    c.space_type ? spaceLabels[c.space_type] : '空间待确认',
    c.target_towns.length ? c.target_towns.join('、') : '镇街待确认',
    c.area_min_sqm !== null && c.area_max_sqm !== null ? `${c.area_min_sqm}-${c.area_max_sqm}㎡` : '面积待确认',
    c.rent_max_cents !== null ? `预算≤¥${formatCentsAsYuan(c.rent_max_cents)}` : '预算面议',
  ]
})
</script>

<template>
  <view class="demand-summary" :class="{ compact }">
    <view class="summary-tags"><text v-for="item in items" :key="item" class="tag gold">{{ item }}</text></view>
    <text v-if="!compact" class="raw">“{{ demand.raw_text }}”</text>
  </view>
</template>

<style scoped>
.summary-tags { display: flex; flex-wrap: wrap; gap: 10rpx; }
.raw { display: block; margin-top: 18rpx; color: #6f5b52; font-size: 24rpx; line-height: 1.65; }
</style>
