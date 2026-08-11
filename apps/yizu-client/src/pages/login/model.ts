import type { DevSessionRequest } from '@/types/api'
import type { DevSessionResponse } from '@/types/domain'
import { isValidPhone } from '@/utils/validation'

export interface MiniappInputEvent {
  detail?: { value?: unknown }
}

export interface MiniappCheckboxEvent {
  detail?: { value?: unknown }
}

export function phoneFromInputEvent(event: unknown): string {
  const value = (event as MiniappInputEvent | null)?.detail?.value
  return typeof value === 'string' || typeof value === 'number' ? String(value).trim() : ''
}

export function codeFromInputEvent(event: unknown): string {
  return phoneFromInputEvent(event).replace(/\D/gu, '').slice(0, 8)
}

export function agreementFromCheckboxEvent(event: unknown): boolean {
  const value = (event as MiniappCheckboxEvent | null)?.detail?.value
  return Array.isArray(value) && value.includes('accepted')
}

export function canSubmitLogin(phone: string, agreed: boolean, submitting: boolean): boolean {
  return isValidPhone(phone) && agreed && !submitting
}

interface DemoLoginDependencies {
  createSession(request: DevSessionRequest): Promise<DevSessionResponse>
  saveSession(session: DevSessionResponse): void
  navigateToAiHome(): void
}

export async function completeDemoLogin(
  phone: string,
  agreed: boolean,
  dependencies: DemoLoginDependencies,
): Promise<void> {
  if (!isValidPhone(phone)) throw new Error('请输入有效的11位中国大陆手机号')
  if (!agreed) throw new Error('请阅读并同意《用户协议》与《隐私政策》')
  const session = await dependencies.createSession({ phone, contact_confirmed: true })
  dependencies.saveSession(session)
  dependencies.navigateToAiHome()
}
