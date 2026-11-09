/** Contracts for shared core helpers that Engine tests do not reach through normal workflows. */
import { describe, expect, it } from 'vitest'
import { MemoryRepositories, UserError, reasonText, type DomainEvent, type Reason, type Trigger } from '../src/core/index.ts'
import { EnginePart, type EngineOptions } from '../src/core/part.ts'
import { required } from '../src/core/required.ts'
import { eventNotice } from '../src/app/notices.ts'
import { settingsPolicy, SettingsSchema } from '../src/policy.ts'
import { SourceRegistry } from '../src/sources/registry.ts'

describe('core helpers', () => {
  it('refuses invariant values that are missing without losing present values', () => {
    expect(required('ticket')).toBe('ticket')
    expect(() => required(undefined)).toThrow('A value required by an invariant is missing')
    expect(() => required(null)).toThrow('A value required by an invariant is missing')
  })

  it('keeps placeholders whose values a caller did not supply', () => {
    expect(new UserError('modelUnavailable').message).toBe('The {block} model is not available: {error}')
  })

  it('records a hand-back that arrives before any disposition exists, without queueing one no ticket decides', () => {
    const repo = new MemoryRepositories()
    const sources = new SourceRegistry(() => SettingsSchema.parse({}))
    const options: EngineOptions = {
      repo,
      policy: () => settingsPolicy(SettingsSchema.parse({}), sources),
      scope: sources,
      assessor: { request: () => {}, release: () => {} },
      executor: { start: () => {}, deliver: () => {}, stop: () => {} },
      outbox: { send: () => {}, status: () => undefined, withdraw: () => {} },
      now: () => '2026-09-09T00:00:00Z',
    }
    const trigger: Trigger = { id: 'trigger-1', source: 'github', threads: [{ source: 'github', id: '1' }],
      actor: { source: 'github', id: 'alice' }, human: true, at: '', text: 'Fix it', meta: {} }
    repo.triggers.save(trigger)
    class HandBack extends EnginePart {
      record(item: Trigger): boolean { return this.handBack('ticket-1', item, 'not mine') }
    }
    expect(new HandBack(options).record(trigger)).toBe(false)
    expect(repo.triggers.disposition('trigger-1')).toMatchObject({
      fanout: [], handbacks: [{ ticketId: 'ticket-1', reason: 'not mine', at: '2026-09-09T00:00:00Z' }],
    })
    expect(repo.threads.get('github:1')).toBeUndefined()
  })

  it('turns domain events into the notices the main Session needs', () => {
    const at = '2026-09-09T00:00:00Z'
    const ended: DomainEvent = { type: 'ticket.ended', at, data: { ticketId: 'ticket-1', by: 'executor', outcome: 'dropped', reason: 'out of scope' } }
    expect(eventNotice(ended)?.[1]).toMatchObject({ ticket_ended: 'ticket-1', by: 'executor', outcome: 'dropped', reason: 'out of scope', at })
    expect(eventNotice(ended, () => undefined)?.[2]).toContain('Ticket ticket-1 ended (out of scope)')
    const requeued: DomainEvent = { type: 'trigger.requeued', at, data: { triggerId: 'trigger-1' } }
    expect(eventNotice(requeued)?.[1]).toMatchObject({ trigger_requeued: 'trigger-1', read: 'trigger/trigger-1', at })
    const person: DomainEvent = { type: 'thread.unsubscribed', at, data: { threadKey: 'github:1', ticketId: 'ticket-1', by: 'person' } }
    expect(eventNotice(person)?.[0]).toBe('subscription:github:1')
    const executor: DomainEvent = { type: 'thread.unsubscribed', at, data: { threadKey: 'github:1', ticketId: 'ticket-1', by: 'executor', reason: 'unrelated' } }
    expect(eventNotice(executor)?.[1]).toMatchObject({ thread_unrelated: { ticket_id: 'ticket-1', thread: 'github:1', reason: 'unrelated' } })
    expect(eventNotice({ type: 'policy.changed', at, data: { from: 'v1', to: 'v2' } })).toBeUndefined()
  })

  it('formats absent, custom, and partial reasons without inventing values', () => {
    expect(reasonText(undefined)).toBe('')
    const custom: Reason = { code: 'custom_code' }
    expect(reasonText(custom)).toBe('custom_code')
    expect(reasonText({ code: 'said' })).toBe('{text}')
    expect(reasonText({ ...custom, triggerId: 'trigger-1' })).toBe('trigger-1: custom_code')
  })
})
