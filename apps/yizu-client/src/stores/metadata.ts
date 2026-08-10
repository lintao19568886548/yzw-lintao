import { defineStore } from 'pinia'
import { miniappApi } from '@/api/miniapp'
import { DONGGUAN_TOWN_FALLBACK } from '@/config/dongguan'
import type { MetadataOptions } from '@/types/domain'

type MetadataLoader = () => Promise<MetadataOptions>

export const useMetadataStore = defineStore('metadata', {
  state: () => ({
    towns: [...DONGGUAN_TOWN_FALLBACK] as string[],
    notice: '',
    loaded: false,
  }),
  actions: {
    async load(selectedTowns: string[] = [], loader: MetadataLoader = () => miniappApi.loadMetadata()): Promise<void> {
      try {
        const metadata = await loader()
        const serverTowns = metadata.towns.filter((town) => DONGGUAN_TOWN_FALLBACK.includes(town))
        this.towns = [...new Set([...serverTowns, ...selectedTowns.filter((town) => DONGGUAN_TOWN_FALLBACK.includes(town))])]
        if (this.towns.length !== DONGGUAN_TOWN_FALLBACK.length) {
          this.towns = [...DONGGUAN_TOWN_FALLBACK]
          this.notice = '镇街选项暂不完整，已启用本地完整列表。'
        } else {
          this.notice = ''
        }
        this.loaded = true
      } catch {
        this.towns = [...new Set([...DONGGUAN_TOWN_FALLBACK, ...selectedTowns.filter((town) => DONGGUAN_TOWN_FALLBACK.includes(town))])]
        this.notice = '镇街选项暂时无法更新，当前使用本地完整列表，不影响继续填写。'
        this.loaded = false
      }
    },
  },
})
