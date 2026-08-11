import { defineStore } from 'pinia'
import { isDemandDraft, isListingSummary } from '@/stores/demand'
import type { DemandHistoryItem, LeadRecord, ListingSummary, MatchResponse } from '@/types/domain'
import { readVersionedJson, removeStored, writeVersionedJson } from '@/utils/storage'

export const HISTORY_STORAGE_KEY = 'yizu-miniapp-demand-history-v1'
const HISTORY_STORAGE_VERSION = 1

interface HistorySnapshot {
  items: DemandHistoryItem[]
}

function isHistoryItem(value: unknown): value is DemandHistoryItem {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  const item = value as Partial<DemandHistoryItem>
  return typeof item.demand_number === 'string'
    && typeof item.lead_number === 'string'
    && typeof item.created_at_epoch_seconds === 'number' && Number.isFinite(item.created_at_epoch_seconds)
    && item.status === 'pending_assignment'
    && typeof item.sla_minutes === 'number' && Number.isFinite(item.sla_minutes)
    && typeof item.masked_phone === 'string'
    && isDemandDraft(item.demand)
    && Array.isArray(item.listings) && item.listings.every(isListingSummary)
}

function isHistorySnapshot(value: unknown): value is HistorySnapshot {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value)
    && Array.isArray((value as Partial<HistorySnapshot>).items)
    && (value as Partial<HistorySnapshot>).items?.every(isHistoryItem) === true
}

export const useHistoryStore = defineStore('history', {
  state: (): HistorySnapshot => ({ items: [] }),
  actions: {
    hydrate(): void {
      const saved = readVersionedJson(HISTORY_STORAGE_KEY, HISTORY_STORAGE_VERSION, isHistorySnapshot)
      if (saved) this.$patch(saved)
    },
    recordSubmission(lead: LeadRecord, maskedPhone: string, response: MatchResponse | null): DemandHistoryItem {
      const selected = new Set(lead.recommended_listing_ids)
      const listings: ListingSummary[] = (response?.matches ?? [])
        .filter((match) => selected.has(match.listing.listing_id))
        .map((match) => match.listing)
      const item: DemandHistoryItem = {
        demand_number: lead.demand_number,
        lead_number: lead.lead_number,
        created_at_epoch_seconds: lead.created_at_epoch_seconds,
        status: lead.status,
        sla_minutes: lead.sla_minutes,
        masked_phone: maskedPhone,
        demand: JSON.parse(JSON.stringify(lead.demand_snapshot)) as DemandHistoryItem['demand'],
        listings,
      }
      this.items = [item, ...this.items.filter((existing) => existing.lead_number !== item.lead_number)].slice(0, 20)
      this.persist()
      return item
    },
    persist(): void {
      writeVersionedJson<HistorySnapshot>(HISTORY_STORAGE_KEY, HISTORY_STORAGE_VERSION, { items: this.items })
    },
    clear(): void {
      this.$reset()
      removeStored(HISTORY_STORAGE_KEY)
    },
  },
})
