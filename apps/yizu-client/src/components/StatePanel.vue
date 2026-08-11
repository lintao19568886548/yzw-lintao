<script setup lang="ts">
withDefaults(defineProps<{ icon?: string; title: string; description: string }>(), { icon: '⌁' })
</script>

<template>
  <view class="card state-panel">
    <view class="state-icon">{{ icon }}</view><text class="section-title">{{ title }}</text>
    <text class="section-subtitle">{{ description }}</text><slot />
  </view>
</template>

<style scoped>
.state-panel { padding: 54rpx 34rpx; text-align: center; }
.state-icon { display: grid; width: 92rpx; height: 92rpx; margin: 0 auto 20rpx; place-items: center; border-radius: 50%; background: #f8edd8; color: #9b6423; font-size: 42rpx; font-weight: 900; }
</style>
