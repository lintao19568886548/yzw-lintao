<script setup lang="ts">
import type { ListingDetail } from '@/types/domain'
withDefaults(defineProps<{ tone?: ListingDetail['image_tone']; caption?: string; compact?: boolean }>(), {
  tone: 'cinnabar', caption: '脱敏示例房源', compact: false,
})
</script>

<template>
  <view class="visual" :class="[tone, { compact }]">
    <view class="sun" /><view class="building"><view /><view /><view /></view><view class="ground" />
    <text class="caption">{{ caption }}</text><text class="privacy">大致位置 · 隐私保护</text>
  </view>
</template>

<style scoped>
.visual { position: relative; overflow: hidden; height: 280rpx; border-radius: 24rpx; background: linear-gradient(155deg, #a5262c, #641017); color: #fff3d1; }
.visual.compact { height: 180rpx; border-radius: 20rpx; }
.visual.gold { background: linear-gradient(155deg, #b67b28, #754519); }.visual.ink { background: linear-gradient(155deg, #4f4a45, #24211f); }.visual.jade { background: linear-gradient(155deg, #427065, #183d37); }
.sun { position: absolute; right: 45rpx; top: 28rpx; width: 78rpx; height: 78rpx; border-radius: 50%; background: rgba(255, 226, 153, .65); box-shadow: 0 0 0 18rpx rgba(255,226,153,.09); }
.building { position: absolute; left: 46rpx; right: 38rpx; bottom: 52rpx; display: flex; align-items: flex-end; gap: 8rpx; }
.building::before { width: 42%; height: 100rpx; content: ''; border: 3rpx solid rgba(255,247,218,.55); background: rgba(255,255,255,.1); transform: skewY(-6deg); }
.building view { flex: 1; height: 70rpx; border: 2rpx solid rgba(255,247,218,.45); background: rgba(255,255,255,.08); }
.building view:nth-child(2) { height: 92rpx; }.building view:nth-child(3) { height: 58rpx; }
.ground { position: absolute; right: 0; bottom: 42rpx; left: 0; height: 2rpx; background: rgba(255,247,218,.4); }
.caption, .privacy { position: absolute; z-index: 2; left: 22rpx; font-size: 21rpx; }
.caption { bottom: 18rpx; font-weight: 800; }.privacy { top: 18rpx; opacity: .78; }
</style>
