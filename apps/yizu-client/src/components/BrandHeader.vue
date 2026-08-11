<script setup lang="ts">
withDefaults(defineProps<{ compact?: boolean; showTrust?: boolean }>(), { compact: false, showTrust: true })
</script>

<template>
  <view class="brand" :class="{ compact }">
    <view class="pattern pattern-a" /><view class="pattern pattern-b" />
    <view class="brand-main">
      <view class="seal"><text>宜</text><text>租</text></view>
      <view class="brand-copy">
        <view class="name-row"><text class="brand-name">宜租网</text><text class="ai-badge">AI 找房</text></view>
        <text class="brand-slogan">企业选址服务 · AI智能匹配</text>
      </view>
    </view>
    <view v-if="showTrust" class="trust-row">
      <text>真实房源 · 专业顾问 · 15分钟响应</text>
    </view>
  </view>
</template>

<style scoped>
.brand { position: relative; overflow: hidden; min-height: 238rpx; padding: 46rpx 34rpx 30rpx; border-radius: 0 0 44rpx 44rpx; background: linear-gradient(135deg, #6d0e18 0%, #a82029 56%, #7b1019 100%); color: #fff4d2; box-shadow: 0 18rpx 42rpx rgba(92, 13, 20, .22); }
.brand.compact { min-height: 174rpx; padding-top: 32rpx; }
.brand-main { position: relative; z-index: 2; display: flex; align-items: center; gap: 22rpx; max-width: 1080rpx; margin: 0 auto; }
.seal { display: grid; flex: 0 0 96rpx; width: 96rpx; height: 96rpx; grid-template-columns: 1fr 1fr; place-items: center; padding: 12rpx; border: 4rpx double #f1c66b; border-radius: 22rpx; background: rgba(74, 4, 11, .25); font-family: "STKaiti", serif; font-size: 29rpx; font-weight: 900; transform: rotate(-2deg); }
.brand-copy { min-width: 0; }
.name-row { display: flex; align-items: center; gap: 16rpx; }
.brand-name { font-family: "STKaiti", "KaiTi", serif; font-size: 52rpx; font-weight: 900; letter-spacing: 7rpx; }
.ai-badge { padding: 6rpx 13rpx; border: 1rpx solid rgba(246, 212, 137, .65); border-radius: 999rpx; background: rgba(255,255,255,.08); color: #f6d489; font-size: 19rpx; font-weight: 800; letter-spacing: 1rpx; }
.brand-slogan { display: block; margin-top: 7rpx; color: #f5dca3; font-size: 22rpx; letter-spacing: 2rpx; }
.trust-row { position: relative; z-index: 2; display: flex; flex-wrap: wrap; gap: 10rpx 20rpx; max-width: 1080rpx; margin: 25rpx auto 0; color: rgba(255, 246, 216, .84); font-size: 19rpx; }
.pattern { position: absolute; border: 3rpx solid rgba(241, 198, 107, .15); border-radius: 50%; }
.pattern::before, .pattern::after { position: absolute; content: ''; border: 3rpx solid rgba(241, 198, 107, .12); border-radius: 50%; }
.pattern-a { right: -54rpx; top: -74rpx; width: 250rpx; height: 250rpx; }
.pattern-a::before { inset: 34rpx; }.pattern-a::after { inset: 70rpx; }
.pattern-b { left: -100rpx; bottom: -175rpx; width: 320rpx; height: 320rpx; }
.pattern-b::before { inset: 44rpx; }.pattern-b::after { inset: 92rpx; }
</style>
