<script setup lang="ts">
defineProps<{ label: string; score: number; reason: string }>()
</script>

<template>
  <view class="score-row">
    <view class="score-label"><text>{{ label }}</text><text>{{ score.toFixed(0) }}</text></view>
    <view class="track"><view class="fill" :style="{ width: `${Math.max(0, Math.min(100, score))}%` }" /></view>
    <text class="reason">{{ reason }}</text>
  </view>
</template>

<style scoped>
.score-row { margin-top: 14rpx; }
.score-label { display: flex; justify-content: space-between; color: #563b31; font-size: 22rpx; }
.track { height: 10rpx; margin-top: 8rpx; overflow: hidden; border-radius: 99rpx; background: #eadfc9; }
.fill { height: 100%; border-radius: inherit; background: linear-gradient(90deg, #b88a32, #981d24); }
.reason { display: block; margin-top: 6rpx; color: #89746a; font-size: 20rpx; }
</style>
