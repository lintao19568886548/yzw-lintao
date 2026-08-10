<script setup lang="ts">
withDefaults(defineProps<{ compact?: boolean }>(), { compact: false })
</script>

<template>
  <view class="brand" :class="{ compact }">
    <view class="seal">宜租</view>
    <view>
      <text class="brand-name">宜租网</text>
      <text class="brand-slogan">东莞产业空间 · AI可信找房</text>
    </view>
    <view class="cloud cloud-left" />
    <view class="cloud cloud-right" />
  </view>
</template>

<style scoped>
.brand { position: relative; overflow: hidden; display: flex; align-items: center; gap: 22rpx; min-height: 210rpx; padding: 48rpx 36rpx; border-radius: 0 0 42rpx 42rpx; background: linear-gradient(135deg, #751018, #a9282d 62%, #8a171e); color: #fff3cf; box-shadow: 0 18rpx 38rpx rgba(96, 13, 20, .25); }
.brand.compact { min-height: 160rpx; }
.seal { z-index: 1; display: grid; width: 96rpx; height: 96rpx; place-items: center; border: 4rpx double #f0c878; border-radius: 22rpx; font-family: serif; font-size: 34rpx; font-weight: 900; transform: rotate(-3deg); }
.brand-name, .brand-slogan { position: relative; z-index: 1; display: block; }
.brand-name { font-family: serif; font-size: 52rpx; font-weight: 900; letter-spacing: 8rpx; }
.brand-slogan { margin-top: 8rpx; color: #f6dfaa; font-size: 23rpx; letter-spacing: 2rpx; }
.cloud { position: absolute; width: 180rpx; height: 70rpx; border: 3rpx solid rgba(240, 200, 120, .25); border-radius: 999rpx; }
.cloud-left { left: -80rpx; bottom: -30rpx; }
.cloud-right { right: -60rpx; top: 20rpx; }
</style>
