<script setup lang="ts">
import { onShow } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import { localDemoMode } from '@/config/runtime'
import { requireAuth } from '@/composables/useAuthGuard'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'

const auth = useAuthStore()
const demand = useDemandStore()
const history = useHistoryStore()

onShow(() => { auth.hydrate(); history.hydrate(); requireAuth(auth) })

function logout(): void {
  uni.showModal({ title: '退出登录', content: '退出后当前未提交的找房流程会清除，历史需求仍保存在本机。', success(result) {
    if (!result.confirm) return
    auth.clear(); demand.resetFlow(); uni.reLaunch({ url: '/pages/login/index' })
  } })
}
</script>

<template>
  <view class="profile-page">
    <BrandHeader compact :show-trust="false" />
    <view class="page-shell content-width">
      <view class="profile-card card"><view class="avatar">企</view><view class="profile-copy"><strong>企业找房用户</strong><text>{{ auth.masked_phone }}</text></view><DemoBadge v-if="localDemoMode" /></view>
      <view class="stats"><view><strong>{{ history.items.length }}</strong><text>已提交需求</text></view><view><strong>15</strong><text>分钟响应目标</text></view><view><strong>3</strong><text>空间类型</text></view></view>
      <view class="card"><text class="section-title">找房服务</text><view class="menu-row"><text class="menu-icon">需</text><view><strong>我的找房需求</strong><small>查看提交记录与当前状态</small></view><text>›</text></view><view class="menu-row"><text class="menu-icon">顾</text><view><strong>顾问服务</strong><small>提交后由专业招商顾问联系</small></view><text>›</text></view></view>
      <view class="card"><text class="section-title">账号与安全</text><view class="menu-row"><text class="menu-icon">隐</text><view><strong>隐私保护</strong><small>不展示真实业主隐私与精确门牌</small></view><text>›</text></view><view class="menu-row"><text class="menu-icon">企</text><view><strong>企业资料</strong><small>专业顾问将协助核验您的找房需求</small></view><text class="tag gold">顾问协助</text></view></view>
      <view class="notice">您的找房需求仅用于空间匹配与顾问服务，我们不会公开展示企业联系方式。</view>
      <button class="danger-button" @click="logout">退出登录</button>
      <text class="version">宜租网——企业选址与空间租赁智能服务平台</text>
    </view>
  </view>
</template>

<style scoped>
.profile-page { min-height: 100vh; }.profile-card { display: flex; align-items: center; gap: 18rpx; margin-top: -10rpx; }.avatar { display: grid; flex: 0 0 82rpx; width: 82rpx; height: 82rpx; place-items: center; border-radius: 24rpx; background: linear-gradient(145deg,#aa2830,#711018); color: #fff2ca; font-family: serif; font-size: 34rpx; font-weight: 900; }.profile-copy { flex: 1; }.profile-copy strong,.profile-copy text { display: block; }.profile-copy strong { color: #4b2723; font-size: 28rpx; }.profile-copy text { margin-top: 6rpx; color: #8a7770; font-size: 22rpx; }
.stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12rpx; margin: 22rpx 0; }.stats view { padding: 21rpx 8rpx; border-radius: 18rpx; background: #fffdf8; text-align: center; }.stats strong,.stats text { display: block; }.stats strong { color: #941d25; font-family: serif; font-size: 34rpx; }.stats text { margin-top: 5rpx; color: #89766f; font-size: 18rpx; }
.menu-row { display: flex; align-items: center; gap: 14rpx; padding: 19rpx 0; border-bottom: 1rpx solid #eee2cf; }.menu-row:last-child { border-bottom: 0; }.menu-icon { display: grid; width: 54rpx; height: 54rpx; place-items: center; border-radius: 15rpx; background: #f7ead2; color: #8f1b23; font-size: 22rpx; font-weight: 900; }.menu-row view { flex: 1; }.menu-row strong,.menu-row small { display: block; }.menu-row strong { color: #50322c; font-size: 24rpx; }.menu-row small { margin-top: 5rpx; color: #8a7770; font-size: 19rpx; }.version { display: block; margin: 24rpx; text-align: center; color: #9a8981; font-size: 19rpx; }
</style>
