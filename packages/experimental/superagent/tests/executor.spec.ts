/** DSH Executor behavior that does not need the full Runtime host. */
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Context } from '@deepseek-ai/cordis'
import { liveTicket, type Assignment, type Ticket, type TicketInput, type Trigger } from '../src/core/index.ts'
import { SettingsSchema } from '../src/policy.ts'
import { DshExecutor, batchLine, batchOf, inputMessage, inputResource, type ExecutorHost } from '../src/executor/dsh.ts'
import { deferred } from './fixtures/async.ts'

let directory: string
beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), 'executor-'))
})
afterEach(() => {
  rmSync(directory, { recursive: true, force: true })
})

const assignment = (id = 'a1', state: Assignment['state'] = 'pending'): Assignment => ({
  id, state, createdAt: '2026-11-01T00:00:00.000Z',
})
/** A minimal open ticket; tests add the state the behavior under test needs. */
const ticket = (id = 't1', input: TicketInput[] = [], undelivered: string[] = []): Ticket => ({
  id,
  title: 'Fix the regression',
  decisionId: 'd1',
  mandate: { strength: 'explicit' },
  channels: [],
  assignments: [assignment()],
  inputs: input,
  undelivered,
  createdAt: '2026-11-01T00:00:00.000Z',
  updatedAt: '2026-11-01T00:00:00.000Z',
  origin: { triggerId: 'tr1', triggerIds: ['tr1', 'tr2', 'gone'] },
}) as Ticket

const trigger = (id: string): Trigger => ({
  id,
  source: 'feishu',
  threads: [{ source: 'feishu', id: 'oc:om_1' }],
  actor: { source: 'feishu', id: 'ou_alice' },
  human: true,
  at: '2026-11-01T00:00:00.000Z',
  text: `Message ${id}`,
  meta: {},
})

type FakeAgent = {
  id: string
  status: string
  whenIdle: () => Promise<void>
  inbox: { remove: (id: string) => void }
  session: { snapshotEvents: () => unknown[] }
  cancel: ReturnType<typeof vi.fn>
}

/** A controllable ExecutorHost, with the launch and delivery effect handlers captured as they are installed. */
function executorHost(seed: { ticket?: Ticket; triggers?: Trigger[] } = {}) {
  const tickets = new Map<string, Ticket>()
  if (seed.ticket) tickets.set(seed.ticket.id, seed.ticket)
  const triggers = new Map(seed.triggers?.map(item => [item.id, item]))
  const agents = new Map<string, FakeAgent>()
  const services = new Map<string, unknown>()
  const healthState = { state: 'ready' as 'ready' | 'error', detail: '' }
  const health = {
    get: () => healthState,
    set: (value: { state: 'ready' | 'error'; detail: string }) => Object.assign(healthState, value),
  }
  const settings = SettingsSchema.parse({ executor: { maxWorkers: 1 } })
  const store = {
    tickets: {
      get: vi.fn((id: string) => tickets.get(id)),
      open: vi.fn(() => [...tickets.values()].filter(liveTicket)),
      list: vi.fn(() => [...tickets.values()]),
    },
    triggers: { get: vi.fn((id: string) => triggers.get(id)) },
    set: vi.fn(),
    settings: vi.fn(() => settings),
    atomic: vi.fn((work: () => void) => { work() }),
    afterCommit: vi.fn((work: () => void) => { work() }),
  }
  const effects = { enqueue: vi.fn(), handle: vi.fn() }
  const sessions = {
    ensure: vi.fn(async (id: string) => agents.get(id) ?? {
      id,
      status: 'idle',
      whenIdle: async () => {},
      inbox: { remove: vi.fn() },
      session: { snapshotEvents: () => [] },
      cancel: vi.fn(),
    }),
    prompt: vi.fn(async () => {}),
    promptedId: vi.fn((id: string) => `prompt-${id}`),
    relay: vi.fn(),
    holds: vi.fn(async () => false),
  }
  const engine = {
    assignmentStarted: vi.fn(),
    inputsDelivered: vi.fn((id: string, _assignmentId: string, inputIds: string[]) => {
      const current = tickets.get(id)!
      current.undelivered = current.undelivered.filter(inputId => !inputIds.includes(inputId))
    }),
    inputsRead: vi.fn(),
    turnEnded: vi.fn(),
    deliveryFailed: vi.fn(),
    report: vi.fn((id: string, report: { state: string; summary: string; at: string }) => {
      const current = tickets.get(id)!
      current.assignments[0]!.report = report
      return current
    }),
    assignmentInterrupted: vi.fn(),
  }
  const sourceLink = vi.fn(async () => ({}))
  const state = { closing: false, paused: false }
  const context = {
    get: (name: string) => services.get(name),
    agents,
    on: vi.fn(),
  } as unknown as Context
  const host: ExecutorHost = {
    context,
    engine,
    store,
    effects,
    health,
    machine: { stateDir: directory },
    lifetime: new AbortController().signal,
    sessions,
    closing: () => state.closing,
    paused: () => state.paused,
    environment: vi.fn(() => []),
    sourceLink,
    repliedTo: vi.fn(() => undefined),
  }
  const executor = new DshExecutor(host)
  return {
    executor, host, tickets, triggers, agents, services, store, effects, sessions, engine,
    health, healthState, state, context, sourceLink,
  }
}

type ExecutorInternals = {
  launchQueued(payload: { ticketId: string; assignmentId: string }): Promise<void>
  launch(ticket: Ticket, assignment: Assignment): Promise<void>
  deliverQueued(ticketId: string): Promise<void>
  resumeAfter(ticketId: string, assignmentId: string, error: unknown): void
  retries(key: string, error: unknown): boolean
  workerIdle(ticket: Ticket, assignment: Assignment, agent: FakeAgent): Promise<void>
  launchFailed(ticketId: string, assignmentId: string, error: unknown): void
  background(work: Promise<void>): Promise<void>
  handOff(ticketId: string, agent: FakeAgent): void
  left: Map<string, { jobs: Set<string>; collect: AbortController }>
}

describe('DshExecutor', () => {
  it('holds both effect queues while processing is paused, and runs them when it resumes', () => {
    const h = executorHost()
    const launch = h.effects.handle.mock.calls.find(([kind]) => kind === 'launch')![2] as { limit: () => number }
    const delivery = h.effects.handle.mock.calls.find(([kind]) => kind === 'delivery')![2] as { limit: () => number }
    h.state.paused = true
    expect(launch.limit()).toBe(0)
    expect(delivery.limit()).toBe(0)
    h.state.paused = false
    expect(launch.limit()).toBe(1)
    expect(delivery.limit()).toBe(4)
  })

  it('delivers nothing for an unknown ticket or a running ticket with nothing undelivered', async () => {
    const item = ticket()
    item.assignments[0]!.state = 'running'
    const h = executorHost({ ticket: item })
    await (h.executor as unknown as ExecutorInternals).deliverQueued('missing')
    await (h.executor as unknown as ExecutorInternals).deliverQueued('t1')
    expect(h.sessions.prompt).not.toHaveBeenCalled()
    expect(h.engine.inputsDelivered).not.toHaveBeenCalled()
  })

  it('reads a long Input through its resource path instead of delivering all of it inline', async () => {
    const text = 'x'.repeat(1600)
    const input = { id: 'i1', kind: 'text', text, at: '' } as TicketInput
    const item = ticket('t1', [input], ['i1'])
    item.assignments[0]!.state = 'running'
    const h = executorHost({ ticket: item })
    await (h.executor as unknown as ExecutorInternals).deliverQueued('t1')
    expect(JSON.parse(h.sessions.prompt.mock.calls[0]![2])).toEqual({
      text: `${'x'.repeat(1500)}…`, chars: 1600, read: expect.stringMatching(/^ticket\/t1\/input\//) as unknown,
    })
  })

  it('shows a worker the reply it has not seen when a Trigger answers an earlier one', async () => {
    const input = { id: 'i1', kind: 'trigger', triggerId: 'tr1', at: '' } as TicketInput
    const item = ticket('t1', [input], ['i1'])
    item.assignments[0]!.state = 'running'
    const parent = trigger('tr0')
    const h = executorHost({ ticket: item, triggers: [trigger('tr1')] })
    h.host.repliedTo = vi.fn(() => parent)
    await (h.executor as unknown as ExecutorInternals).deliverQueued('t1')
    expect(JSON.parse(h.sessions.prompt.mock.calls[0]![2])).toMatchObject({
      replies_to: { trigger_id: 'tr0', preview: 'Message tr0' },
    })
  })

  it('says a legacy policy change generically when the Input carries no effect', () => {
    expect(inputMessage({ id: 'i1', kind: 'policy-changed', at: '', from: 'v1', to: 'v2' })).toBe(
      'Policy change: the access or admission rules changed. Check that the remaining work stays within the assignment.',
    )
  })

  it('names a sender a policy change lost even when the stored effect does not identify it', () => {
    expect(inputMessage({
      id: 'i1', kind: 'policy-changed', at: '', from: 'v1', to: 'v2', effect: { requesterAllowed: false },
    })).toContain('The sender this ticket is for, who asked for it, may no longer start work here')
  })

  it("returns only the other stored triggers of a ticket's origin batch", () => {
    const stored = new Map([['tr2', trigger('tr2')]])
    expect(batchOf({ store: { triggers: { get: (id: string) => stored.get(id) } } } as never, ticket())).toEqual([trigger('tr2')])
  })

  it("renders a batch line and the resource path that carries a ticket Input's full text", () => {
    expect(batchLine(trigger('tr2'))).toEqual({
      trigger_id: 'tr2', from: 'ou_alice', at: '2026-11-01T00:00:00.000Z',
      preview: 'Message tr2', read: 'trigger/tr2',
    })
    expect(inputResource('t1', 'i1')).toMatch(/^ticket\/t1\/input\/[0-9a-f]+$/)
  })

  it("falls back to a trigger's id when its card cannot be read because the Trigger is gone", async () => {
    const input = { id: 'i1', kind: 'trigger', triggerId: 'gone', at: '' } as TicketInput
    const item = ticket('t1', [input], ['i1'])
    item.assignments[0]!.state = 'running'
    const h = executorHost({ ticket: item })
    h.sessions.prompt = vi.fn(async () => { throw new Error('prompt failed') })
    await expect((h.executor as unknown as ExecutorInternals).deliverQueued('t1')).rejects.toThrow('prompt failed')
    expect(h.sourceLink).not.toHaveBeenCalled()
    expect(h.sessions.prompt.mock.calls[0]![2]).toBe('{"trigger_id":"gone","read":"trigger/gone"}')
  })

  it('delivers no legacy reopen Input, and marks it delivered without prompting the worker', async () => {
    const reopen = { id: 'i1', kind: 'reopen', decisionId: 'd1', at: '' } as TicketInput
    const item = ticket('t1', [reopen], ['i1'])
    item.assignments[0]!.state = 'running'
    const h = executorHost({ ticket: item })
    await (h.executor as unknown as ExecutorInternals).deliverQueued('t1')
    expect(h.sessions.prompt).not.toHaveBeenCalled()
    expect(h.engine.inputsDelivered).toHaveBeenCalledWith('t1', 'a1', ['i1'])
  })

  it('records and rethrows a delivery failure while the ticket stays live', async () => {
    const input = { id: 'i1', kind: 'trigger', triggerId: 'tr1', at: '' } as TicketInput
    const item = ticket('t1', [input], ['i1'])
    item.assignments[0]!.state = 'running'
    const h = executorHost({ ticket: item, triggers: [trigger('tr1')] })
    h.sessions.prompt = vi.fn(async () => { throw new Error('provider rejected') })
    await expect((h.executor as unknown as ExecutorInternals).deliverQueued('t1')).rejects.toThrow('provider rejected')
    expect(h.engine.deliveryFailed).toHaveBeenCalledWith('t1', 'a1', 'Error: provider rejected')
  })

  it('stops resuming a transient worker failure when the host is closing by the time its backoff ends', async () => {
    vi.useFakeTimers()
    try {
      const item = ticket()
      item.assignments[0]!.state = 'running'
      const h = executorHost({ ticket: item })
      const internal = h.executor as unknown as ExecutorInternals
      internal.retries('a1', new Error('fetch failed'))
      internal.resumeAfter('t1', 'a1', new Error('fetch failed'))
      h.state.closing = true
      await vi.advanceTimersByTimeAsync(15_000)
      expect(h.sessions.prompt).not.toHaveBeenCalled()
    } finally { vi.useRealTimers() }
  })

  it('waits for a worker slot when a transient failure resumes while every worker runs', async () => {
    vi.useFakeTimers()
    try {
      const item = ticket('t1')
      item.assignments[0]!.state = 'running'
      const busy = ticket('t2')
      busy.assignments[0]!.state = 'running'
      const h = executorHost({ ticket: item })
      h.tickets.set('t2', busy)
      h.agents.set('worker-t2', {
        id: 'worker-t2', status: 'running', whenIdle: async () => {}, inbox: { remove: vi.fn() },
        session: { snapshotEvents: () => [] }, cancel: vi.fn(),
      })
      const internal = h.executor as unknown as ExecutorInternals
      internal.retries('a1', new Error('fetch failed'))
      internal.resumeAfter('t1', 'a1', new Error('fetch failed'))
      await vi.advanceTimersByTimeAsync(15_000)
      expect(h.sessions.prompt).not.toHaveBeenCalled()
      h.agents.delete('worker-t2')
      await vi.advanceTimersByTimeAsync(5_000)
      expect(h.sessions.prompt).toHaveBeenCalledWith('worker-t1', 'retry:a1:1', expect.any(String), 'queue')
    } finally { vi.useRealTimers() }
  })

  it('marks the ticket failed when a resumed worker prompt fails', async () => {
    vi.useFakeTimers()
    try {
      const item = ticket()
      item.assignments[0]!.state = 'running'
      const h = executorHost({ ticket: item })
      h.sessions.prompt = vi.fn(async () => { throw new Error('provider rejected') })
      const internal = h.executor as unknown as ExecutorInternals
      internal.retries('a1', new Error('fetch failed'))
      internal.resumeAfter('t1', 'a1', new Error('fetch failed'))
      await vi.advanceTimersByTimeAsync(15_000)
      await vi.waitFor(() => {
        expect(item.assignments[0]!.report).toMatchObject({ state: 'failed', summary: 'Error: provider rejected' })
      })
    } finally { vi.useRealTimers() }
  })

  it('returns from a launch once the ticket ends while its worker session is being created', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    const h = executorHost({ ticket: item })
    const gate = deferred<undefined>()
    h.sessions.ensure = vi.fn(async () => {
      item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
      await gate.promise
      return {
        id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { remove: vi.fn() },
        session: { snapshotEvents: () => [] }, cancel: vi.fn(),
      }
    })
    const launch = (h.executor as unknown as ExecutorInternals).launchQueued({ ticketId: 't1', assignmentId: 'a1' })
    gate.resolve(undefined)
    await launch
    expect(h.engine.assignmentStarted).not.toHaveBeenCalled()
  })

  it('returns from a launch once the ticket ends while it asks whether the session already holds the assignment', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    item.assignments.push({ ...assignment('a2'), state: 'pending' })
    const h = executorHost({ ticket: item })
    h.sessions.holds = vi.fn(async () => {
      item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
      return false
    })
    await (h.executor as unknown as ExecutorInternals).launchQueued({ ticketId: 't1', assignmentId: 'a2' })
    expect(h.sessions.prompt).not.toHaveBeenCalled()
  })

  it('returns from a launch once the ticket ends while the source trigger card is being read', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    const h = executorHost({ ticket: item, triggers: [trigger('tr1')] })
    h.host.sourceLink = vi.fn(async () => {
      item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
      return {}
    })
    h.sessions.prompt = vi.fn(async () => { throw new Error('The ticket ended or another Assignment took it over') })
    await (h.executor as unknown as ExecutorInternals).launchQueued({ ticketId: 't1', assignmentId: 'a1' })
    expect(h.sessions.prompt).not.toHaveBeenCalled()
  })

  it('rejects a launch whose authorization changes before the worker takes the assignment', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    item.assignments.push({ ...assignment('a2'), state: 'pending' })
    const h = executorHost({ ticket: item })
    h.sessions.holds = vi.fn(async () => true)
    h.sessions.prompt = vi.fn(async (_id: string, _requestId: string, _text: string, _mode: string, authorize?: () => void) => {
      item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
      authorize?.()
    })
    await expect((h.executor as unknown as ExecutorInternals).launch(item, item.assignments.at(-1)!))
      .rejects.toThrow('Launch authorization changed before worker delivery')
    expect(h.engine.assignmentStarted).not.toHaveBeenCalled()
  })

  it('marks no stale ticket failed when a launch failure arrives after the ticket ended', () => {
    const item = ticket()
    item.assignments[0]!.state = 'pending'
    item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
    const h = executorHost({ ticket: item })
    ;(h.executor as unknown as ExecutorInternals).launchFailed('t1', 'a1', new Error('provider rejected'))
    expect(item.assignments[0]!.report).toBeUndefined()
  })

  it('recovers the worker health entry after background work succeeds again', async () => {
    const h = executorHost()
    h.healthState.state = 'error'
    h.healthState.detail = 'earlier failure'
    await (h.executor as unknown as ExecutorInternals).background(Promise.resolve())
    expect(h.healthState).toMatchObject({ state: 'ready', detail: 'Worker events are handled' })
  })

  it('does not resume a transient failure whose ticket ended by the time its backoff ends', async () => {
    vi.useFakeTimers()
    try {
      const item = ticket()
      item.assignments[0]!.state = 'running'
      const h = executorHost({ ticket: item })
      const internal = h.executor as unknown as ExecutorInternals
      internal.retries('a1', new Error('fetch failed'))
      internal.resumeAfter('t1', 'a1', new Error('fetch failed'))
      item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
      await vi.advanceTimersByTimeAsync(15_000)
      expect(h.sessions.prompt).not.toHaveBeenCalled()
    } finally { vi.useRealTimers() }
  })

  it('prompts no end for a job left to a person once its collection is aborted', async () => {
    const item = ticket()
    const agent = {
      id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { remove: vi.fn() },
      session: { snapshotEvents: () => [] }, cancel: vi.fn(),
    }
    const h = executorHost({ ticket: item })
    h.agents.set('worker-t1', agent)
    h.services.set('jobs', {
      list: () => [{ id: 'bash-1', owner: 'worker-t1', status: 'running', label: 'pnpm preview' }],
      wait: (_id: string, _timeoutMs: number, _caller: string, signal?: AbortSignal) =>
        new Promise((_resolve, reject) => { signal?.addEventListener('abort', () => { reject(new Error('wait aborted')) }, { once: true }) }),
      kill: () => {},
      events: { subscribe: () => () => {} },
    })
    const internal = h.executor as unknown as ExecutorInternals
    internal.handOff('t1', agent)
    internal.left.get('t1')!.collect.abort()
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(h.sessions.prompt).not.toHaveBeenCalled()
  })

  it('does not launch a ticket that ended before its launch checks the workspace', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
    const h = executorHost({ ticket: item })
    await (h.executor as unknown as ExecutorInternals).launch(item, item.assignments[0]!)
    expect(h.sessions.ensure).not.toHaveBeenCalled()
  })

  it('does not launch a ticket that ends while a previous stop of its background work runs', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    const h = executorHost({ ticket: item })
    const stop = deferred<undefined>()
    h.services.set('jobs', {
      list: () => [{ id: 'bash-1', owner: 'worker-t1', status: 'running' }],
      wait: () => stop.promise.then(() => ({ status: 'completed' })),
      kill: () => {},
      events: { subscribe: () => () => {} },
    })
    const internal = h.executor as unknown as ExecutorInternals
    h.executor.stop(item, item.assignments[0]!, 'Assignment replaced')
    const launch = internal.launch(item, item.assignments[0]!)
    await new Promise(resolve => setTimeout(resolve, 0))
    item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
    stop.resolve(undefined)
    await launch
    expect(h.sessions.prompt).not.toHaveBeenCalled()
  })

  it('prepares a new workspace and continues the launch once it verifies', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'new-fix'), assignment: 'Fix it.' }
    const h = executorHost({ ticket: item })
    const gate = deferred<undefined>()
    const subprocess = {
      spawn: vi.fn(() => ({
        done: gate.promise.then(() => ({ exitCode: 0 })),
        collected: { stdout: { readFrom: () => ({ text: '' }) }, stderr: { readFrom: () => ({ text: '' }) } },
      })),
    }
    h.services.set('subprocess', subprocess)
    ;(h.context as { subprocess?: unknown }).subprocess = subprocess
    const launch = (h.executor as unknown as ExecutorInternals).launch(item, item.assignments[0]!)
    await new Promise(resolve => setTimeout(resolve, 0))
    gate.resolve(undefined)
    await launch
    expect(h.sessions.ensure).toHaveBeenCalledWith('worker-t1', item.execution.directory, item.title, 'worker')
  })

  it('stops a launch whose ticket ends while its new workspace is being prepared', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'stopped-fix'), assignment: 'Fix it.' }
    const h = executorHost({ ticket: item })
    const gate = deferred<undefined>()
    const subprocess = {
      spawn: vi.fn(() => ({
        done: gate.promise.then(() => ({ exitCode: 0 })),
        collected: { stdout: { readFrom: () => ({ text: '' }) }, stderr: { readFrom: () => ({ text: '' }) } },
      })),
    }
    h.services.set('subprocess', subprocess)
    ;(h.context as { subprocess?: unknown }).subprocess = subprocess
    const launch = (h.executor as unknown as ExecutorInternals).launch(item, item.assignments[0]!)
    await new Promise(resolve => setTimeout(resolve, 0))
    item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
    gate.resolve(undefined)
    await launch
    expect(h.sessions.ensure).not.toHaveBeenCalled()
  })

  it('stops a launch whose ticket ends between its workspace check and worker creation', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    const ended = { ...item, ended: { at: '2026-11-02T00:00:00.000Z', by: 'human' as const, outcome: 'stopped' as const } }
    const h = executorHost({ ticket: item })
    let reads = 0
    h.store.tickets.get = vi.fn(() => reads++ === 0 ? item : ended)
    await (h.executor as unknown as ExecutorInternals).launch(item, item.assignments[0]!)
    expect(h.sessions.ensure).not.toHaveBeenCalled()
  })

  it('tells a retried Assignment to continue from the current state', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: '' }
    mkdirSync(item.execution.directory, { recursive: true })
    item.assignments[0]!.state = 'ended'
    item.assignments[0]!.endedReason = 'retried'
    item.assignments.push(assignment('a2'))
    const h = executorHost({ ticket: item })
    await (h.executor as unknown as ExecutorInternals).launch(item, item.assignments[1]!)
    expect(h.sessions.prompt.mock.calls[0]![2]).toContain('A person retried this ticket')
  })

  it('prompts a later assignment with only what is new when the session already holds the first assignment', async () => {
    const item = ticket()
    item.execution = { name: 'fix', directory: join(directory, 'fix'), assignment: 'Fix it.' }
    mkdirSync(item.execution.directory, { recursive: true })
    item.assignments.push({ ...assignment('a2'), state: 'pending' })
    const h = executorHost({ ticket: item })
    h.sessions.holds = vi.fn(async () => true)
    h.sessions.prompt = vi.fn(async (_id: string, _requestId: string, _text: string, _mode: string, before?: () => void) => {
      before?.()
    })
    await (h.executor as unknown as ExecutorInternals).launchQueued({ ticketId: 't1', assignmentId: 'a2' })
    expect(h.sessions.prompt).toHaveBeenCalledWith('worker-t1', 'assignment:t1:a2',
      expect.stringContaining('"assignment":2'), 'queue', expect.any(Function), expect.any(Function))
    expect(h.engine.assignmentStarted).toHaveBeenCalledWith('t1', 'a2')
  })

  it('hands a worker back to a person whose attention appears while its background work ends', async () => {
    const item = ticket()
    item.assignments[0]!.state = 'running'
    const agent = {
      id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { remove: vi.fn() },
      session: { snapshotEvents: () => [] }, cancel: vi.fn(),
    }
    const h = executorHost({ ticket: item })
    h.agents.set('worker-t1', agent)
    const children = deferred<undefined>()
    h.services.set('subagents', {
      waitForChildren: () => children.promise.then(() => false),
      listChildren: async () => [],
      drainChildren: async () => {},
    })
    const idle = (h.executor as unknown as ExecutorInternals).workerIdle(item, item.assignments[0]!, agent)
    await new Promise(resolve => setTimeout(resolve, 0))
    item.attention = { id: 'a1', question: 'Which branch?' }
    children.resolve(undefined)
    await idle
    expect(h.engine.turnEnded).toHaveBeenCalledWith('t1', 'a1')
    expect(item.assignments[0]!.report).toBeUndefined()
  })

  it('reports the last assistant message when an idle worker without a report becomes waiting', async () => {
    const item = ticket()
    item.assignments[0]!.state = 'running'
    const agent = {
      id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { remove: vi.fn() },
      session: {
        snapshotEvents: () => [{
          type: 'user/message',
        }, {
          type: 'assistant/message',
          data: { message: { content: [{ type: 'text', text: 'Ready soon' }] } },
        }],
      },
      cancel: vi.fn(),
    }
    const h = executorHost({ ticket: item })
    h.agents.set('worker-t1', agent)
    await (h.executor as unknown as ExecutorInternals).workerIdle(item, item.assignments[0]!, agent)
    expect(item.assignments[0]!.report).toMatchObject({ state: 'waiting', summary: 'Ready soon' })
    expect(h.engine.turnEnded).toHaveBeenCalledWith('t1', 'a1')
  })

  it('does not turn an ended ticket waiting when its worker goes idle', async () => {
    const item = ticket()
    item.assignments[0]!.state = 'running'
    item.ended = { at: '2026-11-02T00:00:00.000Z', by: 'human', outcome: 'stopped' }
    const agent = {
      id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { remove: vi.fn() },
      session: { snapshotEvents: () => [] }, cancel: vi.fn(),
    }
    const h = executorHost({ ticket: item })
    h.agents.set('worker-t1', agent)
    await (h.executor as unknown as ExecutorInternals).workerIdle(item, item.assignments[0]!, agent)
    expect(h.engine.turnEnded).not.toHaveBeenCalled()
    expect(item.assignments[0]!.report).toBeUndefined()
  })
})
