<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { onShow } from '@dcloudio/uni-app'
import BrandHeader from '@/components/BrandHeader.vue'
import DemoBadge from '@/components/DemoBadge.vue'
import DemandSummary from '@/components/DemandSummary.vue'
import { miniappApi } from '@/api/miniapp'
import { isSessionError, mapApiError } from '@/api/client'
import { localDemoMode } from '@/config/runtime'
import { requireAuth } from '@/composables/useAuthGuard'
import { restoreFlowState, useDemandForm } from '@/composables/useDemandForm'
import { useAuthStore } from '@/stores/auth'
import { useDemandStore } from '@/stores/demand'
import { useHistoryStore } from '@/stores/history'
import { useMetadataStore } from '@/stores/metadata'
import type { SpaceType } from '@/types/domain'
import { validateInitialDemand } from '@/utils/validation'

const auth = useAuthStore()
const demandStore = useDemandStore()
const history = useHistoryStore()
const metadata = useMetadataStore()
const { rawText, selectedType, selectedTown, areaMin, areaMax, budgetYuan, syncFromStore, applyHomeToStore } = useDemandForm(demandStore)
const loading = ref(false)
const errorMessage = ref('')
const popularTowns = ['松山湖', '南城', '长安', '塘厦', '寮步', '大朗']
const typeOptions: Array<{ value: SpaceType; label: string; icon: string; desc: string }> = [
  { value: 'factory', label: '找厂房', icon: '厂', desc: '生产制造 · 用电消防' },
  { value: 'warehouse', label: '找仓库', icon: '仓', desc: '仓储物流 · 装卸通行' },
  { value: 'office', label: '找写字楼', icon: '办', desc: '研发办公 · 商务配套' },
]
const quickTags = ['1500㎡左右', '预算每月4万元', '一个月内入驻', '需要500kVA用电', '最好有3吨货梯']
const recent = computed(() => history.items.slice(0, 2))

watch([rawText, selectedType, selectedTown, areaMin, areaMax, budgetYuan], () => {
  try { applyHomeToStore() } catch { /* 输入完成前不覆盖有效缓存。 */ }
})

onShow(async () => {
  restoreFlowState(auth, demandStore, syncFromStore)
  history.hydrate()
  if (!requireAuth(auth)) return
  await metadata.load(demandStore.demand.constraints.target_towns)
})

function chooseType(type: SpaceType, label: string): void {
  selectedType.value = type
  if (!rawText.value.trim()) rawText.value = `我想在东莞${label.replace('找', '找一处')}`
}

function addQuickTag(tag: string): void {
  const separator = rawText.value.trim() ? '，' : ''
  if (!rawText.value.includes(tag)) rawText.value = `${rawText.value.trim()}${separator}${tag}`
}

function chooseTown(town: string): void {
  selectedTown.value = town
  if (!rawText.value.includes(town)) rawText.value = `${rawText.value.trim()}${rawText.value.trim() ? '，' : ''}目标在${town}`
}

function reuseDemand(index: number): void {
  const item = recent.value[index]
  if (!item) return
  demandStore.prepareDemand(item.demand)
  syncFromStore()
  uni.pageScrollTo({ scrollTop: 0, duration: 250 })
}

async function interpret(): Promise<void> {
  if (!requireAuth(auth)) return
  try { applyHomeToStore() } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : '预算格式无效'; return
  }
  const errors = validateInitialDemand(demandStore.demand)
  if (errors.length) { errorMessage.value = errors.join('；'); return }
  loading.value = true
  errorMessage.value = ''
  demandStore.interpreting = true
  try {
    const interpretation = await miniappApi.interpretDemand({ session_token: auth.session_token, draft: demandStore.demand })
    demandStore.setInterpretation(interpretation)
    uni.navigateTo({ url: '/pages/confirm/index' })
  } catch (error) {
    if (isSessionError(error)) { auth.clear(); requireAuth(auth); return }
    errorMessage.value = mapApiError(error)
  } finally {
    loading.value = false
    demandStore.interpreting = false
  }
}
</script>

<template>
  <view class="home-page">
    <BrandHeader />
    <view class="page-shell content-width">
      <view class="hero-copy">
        <view class="hero-badges"><DemoBadge v-if="localDemoMode" /><text class="tag green">东莞全市服务</text></view>
        <text class="page-kicker">企业选址服务 · AI智能匹配</text>
        <text class="hero-title">说出需求，<text>AI 帮你找空间</text></text>
        <text class="page-desc">厂房 · 仓库 · 写字楼，一句话描述，最快 1 分钟获得专业匹配建议。</text>
      </view>

      <view class="card ai-box">
        <view class="ai-label"><text class="ai-orb">AI</text><text>描述你的找房需求</text></view>
        <textarea v-model="rawText" class="textarea demand-input" maxlength="1000" placeholder="例如：我想在松山湖找1500㎡左右的厂房，需要500kVA用电，最好有3吨货梯，预算每月4万元，一个月内入驻。" />
        <view class="counter-row"><text>支持文字描述与快捷条件组合</text><text>{{ rawText.length }}/1000</text></view>
        <view class="chips quick-tags"><button v-for="tag in quickTags" :key="tag" class="chip" @click="addQuickTag(tag)">+ {{ tag }}</button></view>
        <view v-if="errorMessage" class="error-panel">{{ errorMessage }}</view>
        <button class="primary-button send-button" :disabled="loading" @click="interpret"><text>{{ loading ? 'AI 正在理解需求…' : '发送需求 · 开始智能匹配' }}</text></button>
        <button v-if="errorMessage" class="ghost-button" :disabled="loading" @click="interpret">重新尝试</button>
      </view>

      <view class="section-block">
        <view class="section-heading"><view><text class="section-title">快速找空间</text><text class="section-subtitle">先选类型，再补充关键条件</text></view></view>
        <view class="space-grid">
          <button v-for="item in typeOptions" :key="item.value" class="space-card" :class="{ active: selectedType === item.value }" @click="chooseType(item.value, item.label)">
            <text class="space-icon">{{ item.icon }}</text><text class="space-label">{{ item.label }}</text><text class="space-desc">{{ item.desc }}</text>
          </button>
        </view>
      </view>

      <view class="card quick-form">
        <text class="section-title">热门镇街与基础条件</text>
        <view class="chips"><button v-for="town in popularTowns" :key="town" class="chip" :class="{ active: selectedTown === town }" @click="chooseTown(town)">{{ town }}</button></view>
        <view class="row field">
          <view><text class="field-label">面积下限（㎡）</text><input v-model.number="areaMin" class="input" type="number" placeholder="如 1200" /></view>
          <view><text class="field-label">面积上限（㎡）</text><input v-model.number="areaMax" class="input" type="number" placeholder="如 1800" /></view>
        </view>
        <view class="field"><text class="field-label">月租预算（元）</text><input v-model="budgetYuan" class="input" type="digit" placeholder="可选，最多两位小数" /></view>
        <view v-if="metadata.notice" class="notice">{{ metadata.notice }}</view>
      </view>

      <view v-if="recent.length" class="section-block">
        <text class="section-title">最近需求</text>
        <view v-for="(item, index) in recent" :key="item.lead_number" class="card recent-card" @click="reuseDemand(index)">
          <view class="recent-head"><text>{{ item.demand_number }}</text><text class="tag gold">再次找房</text></view>
          <DemandSummary :demand="item.demand" compact />
        </view>
      </view>

      <view class="card process-card">
        <text class="section-title">推荐找房步骤</text>
        <view class="steps">
          <view><text>01</text><strong>描述需求</strong><small>一句话说清大概想法</small></view>
          <view><text>02</text><strong>确认条件</strong><small>硬条件与偏好由你决定</small></view>
          <view><text>03</text><strong>智能推荐</strong><small>只推荐符合资格的房源</small></view>
          <view><text>04</text><strong>顾问跟进</strong><small>工作时间 15 分钟响应</small></view>
        </view>
      </view>
      <view class="promise"><text class="promise-seal">15</text><view><strong>招商顾问将在工作时间15分钟内联系</strong><small>当前本地演示不会产生真实电话、短信或通知</small></view></view>
    </view>
  </view>
</template>

<style scoped>
.home-page { min-height: 100vh; background: linear-gradient(180deg, #f7efe0 0, #f6f3ed 540rpx); }
.hero-copy { padding: 8rpx 6rpx 26rpx; }.hero-badges { display: flex; flex-wrap: wrap; gap: 10rpx; margin-bottom: 22rpx; }
.hero-title { display: block; color: #3c2520; font-family: "STKaiti", serif; font-size: 52rpx; font-weight: 900; line-height: 1.22; }.hero-title text { color: #991c25; }
.ai-box { position: relative; padding: 28rpx; border-top: 5rpx solid #a51f28; }.ai-label { display: flex; align-items: center; gap: 12rpx; margin-bottom: 18rpx; color: #512f29; font-size: 26rpx; font-weight: 900; }
.ai-orb { display: grid; width: 52rpx; height: 52rpx; place-items: center; border-radius: 16rpx; background: linear-gradient(135deg,#bd3a35,#781019); color: #fff5d9; font-size: 19rpx; box-shadow: 0 7rpx 16rpx rgba(130,18,24,.22); }
.demand-input { min-height: 260rpx; border: 0; background: #faf6ed; }.counter-row { display: flex; justify-content: space-between; margin-top: 10rpx; color: #9a877e; font-size: 19rpx; }.quick-tags { margin-top: 20rpx; }.send-button { margin-top: 26rpx; }
.section-block { margin-top: 36rpx; }.space-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 14rpx; }
.space-card { display: flex; min-width: 0; margin: 0; padding: 24rpx 10rpx; flex-direction: column; align-items: center; border: 1rpx solid #e4d7c0; border-radius: 22rpx; background: #fffdf8; line-height: 1.4; }.space-card.active { border-color: #a51f28; background: #fff4ed; }
.space-icon { display: grid; width: 64rpx; height: 64rpx; place-items: center; border-radius: 18rpx; background: #f5e7ca; color: #8d1d24; font-family: serif; font-size: 30rpx; font-weight: 900; }.space-label { margin-top: 13rpx; color: #4c2a25; font-size: 25rpx; font-weight: 900; }.space-desc { margin-top: 6rpx; color: #8b7770; font-size: 17rpx; }
.quick-form { margin-top: 26rpx; }.recent-card { margin-top: 14rpx; }.recent-head { display: flex; justify-content: space-between; margin-bottom: 16rpx; color: #5b3430; font-size: 24rpx; font-weight: 800; }
.process-card { margin-top: 34rpx; }.steps { display: grid; grid-template-columns: repeat(2, 1fr); gap: 14rpx; }.steps view { position: relative; padding: 20rpx; border-radius: 18rpx; background: #faf5eb; }.steps text { color: #bc8d3d; font-family: serif; font-size: 27rpx; font-weight: 900; }.steps strong, .steps small { display: block; }.steps strong { margin-top: 8rpx; color: #57342d; font-size: 24rpx; }.steps small { margin-top: 5rpx; color: #8a766f; font-size: 19rpx; }
.promise { display: flex; align-items: center; gap: 20rpx; margin: 26rpx 0; padding: 26rpx; border-radius: 24rpx; background: linear-gradient(135deg,#80131b,#a9262d); color: #fff3d2; }.promise-seal { display: grid; flex: 0 0 80rpx; width: 80rpx; height: 80rpx; place-items: center; border: 3rpx double #efc76c; border-radius: 50%; font-family: serif; font-size: 34rpx; font-weight: 900; }.promise strong,.promise small { display: block; }.promise strong { font-size: 24rpx; }.promise small { margin-top: 7rpx; color: #efdca9; font-size: 19rpx; }
</style>
