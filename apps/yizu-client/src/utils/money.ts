export const MAX_BUDGET_CENTS = 10_000_000_000

export class MoneyInputError extends Error {}

export function yuanInputToCents(value: string | null | undefined): number | null {
  const normalized = value?.trim() ?? ''
  if (normalized === '') return null
  if (normalized.startsWith('-')) throw new MoneyInputError('金额不能为负数')
  if (!/^\d+(?:\.\d{1,2})?$/u.test(normalized)) {
    throw new MoneyInputError('金额必须是整数或最多两位小数')
  }
  const [yuan, fraction = ''] = normalized.split('.')
  const cents = Number(yuan) * 100 + Number(fraction.padEnd(2, '0'))
  if (!Number.isSafeInteger(cents) || cents > MAX_BUDGET_CENTS) {
    throw new MoneyInputError('金额不能超过100000000元')
  }
  return cents
}

export function centsToYuanInput(cents: number | null | undefined): string {
  if (cents === null || cents === undefined) return ''
  if (!Number.isSafeInteger(cents) || cents < 0 || cents > MAX_BUDGET_CENTS) {
    throw new MoneyInputError('接口金额不是有效的整数分')
  }
  const yuan = Math.floor(cents / 100)
  const fraction = String(cents % 100).padStart(2, '0').replace(/0+$/u, '')
  return fraction ? `${yuan}.${fraction}` : String(yuan)
}

export function formatCentsAsYuan(cents: number): string {
  const input = centsToYuanInput(cents)
  const [yuan, fraction] = input.split('.')
  const grouped = Number(yuan).toLocaleString('zh-CN')
  return fraction ? `${grouped}.${fraction}` : grouped
}
