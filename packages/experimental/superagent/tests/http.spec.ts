import { expect, it, vi } from 'vitest'
import { z } from 'zod'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { registerHttp } from '../src/app/http.ts'
import { PromptConflict } from '../src/app/prompts.ts'
import { SettingsConflict } from '../src/app/settings.ts'
import type { HttpHost } from '../src/app/http.ts'
import { SettingsSchema } from '../src/policy.ts'
import { SetupError } from '../src/setup.ts'
import { Store } from '../src/app/store.ts'
import { Prompts, defaultPrompts } from '../src/app/prompts.ts'
import { TextCache } from '../src/app/cache.ts'
import { githubKind } from '../src/sources/github.ts'
import { feishuKind } from '../src/sources/feishu.ts'
import { SourceRegistry } from '../src/sources/registry.ts'
import { testDeployment, testMachine } from './fixtures/deployment.ts'

type Handler = (req: unknown, res: unknown) => Promise<void>
type Headers = Record<string, string | undefined>
/** Mount management HTTP routes on a fake web server and return a request helper that includes default panel headers. */
function serve(runtime: Partial<Record<keyof HttpHost, unknown>>, host = '127.0.0.1') {
  let handler: Handler = async () => {}
  const ctx = { webServer: { host, port: 3082, register: (route: { handler: Handler }) => { handler = route.handler; return () => {} } } }
  registerHttp(ctx as never, { ready: Promise.resolve(), ...runtime } as unknown as HttpHost)
  return async (method: string | undefined, path: string | undefined, body?: unknown, headers: Headers = {}) => {
    let status = 0, text = ''
    const chunks = body === undefined ? [] : [Buffer.from(typeof body === 'string' ? body : JSON.stringify(body))]
    const sent = Object.fromEntries(Object.entries({ host: '127.0.0.1:3082', 'x-orchestrator-request': '1', ...headers }).filter(([,
      value]) => value !== undefined))
    const req = { method, url: path === undefined ? undefined : `/orchestrator/api/${path}`, headers: sent, async *[Symbol.asyncIterator]() { yield* chunks } }
    await handler(req, { writeHead: (code: number) => { status = code }, end: (data: string) => { text = data } })
    return { status, body: text ? JSON.parse(text) as Record<string, unknown> : undefined }
  }
}

it('answers each kind of failure with its own status', async () => {
  const saves: unknown[] = []
  const call = serve({
    store: { tickets: { get: () => undefined }, blockRevision: () => 0 },
    sources: { instances: () => [{ id: 'github' }] },
    settings: {
      save: async (...args: unknown[]) => {
        saves.push(args)
        if (args[1] === 'stale') throw new SettingsConflict('Settings changed. Reload before saving.')
        if (args[1] === 'invalid') z.object({ maxWorkers: z.number() }).parse({})
        if (args[1] === 'refused') throw new SetupError('agentCliUnset')
        if (args[1] === 'broken') throw new Error('disk full')
        if (args[1] === 'thrown') throw 'worker queue closed'
        return { saved: true, revision: 1, restarting: false }
      },
    },
  })
  expect(await call('PUT', 'settings/executor', { value: 'stale', revision: 0 })).toMatchObject({ status: 409 })
  expect(await call('PUT', 'settings/executor', { value: 'invalid', revision: 0 })).toMatchObject({
    status: 400,
    body: { issues: [{ path: 'maxWorkers' }] },
  })
  expect(await call('PUT', 'settings/source:github', { value: 'refused', revision: 0 })).toMatchObject({
    status: 400,
    body: { setup: { code: 'agentCliUnset', place: 'github' } },
  })
  expect(await call('PUT', 'settings/executor', { value: 'broken', revision: 0 })).toEqual({ status: 500, body: { error: 'disk full' } })
  expect(await call('PUT', 'settings/executor', { value: 'thrown', revision: 0 })).toEqual({ status: 500, body: { error: 'worker queue closed' } })
  // Block names are validated before calling settings.save: reject unknown blocks or non-existent Sources.
  expect(await call('PUT', 'settings/pause', { value: {}, revision: 0 })).toMatchObject({ status: 400 })
  expect(await call('PUT', 'settings/source:feishu', { value: {}, revision: 0 })).toEqual({
    status: 404,
    body: { error: 'Source not found' },
  })
  expect(saves).toHaveLength(5)
  expect(await call('POST', 'tickets/t9/retry')).toEqual({ status: 404, body: { error: 'Ticket not found' } })
  expect(await call('GET', 'nothing/here')).toEqual({ status: 404, body: { error: 'Unknown operation' } })
  expect(await call('POST', 'executor/pause', 'not json')).toEqual({ status: 400, body: { error: 'The request body is not JSON' } })
})

it('keeps management requests inside the API and defaults the request method', async () => {
  const call = serve({})
  expect(await call('GET', '../panel')).toEqual({ status: 404, body: undefined })
  expect(await call(undefined, undefined)).toEqual({ status: 404, body: undefined })
  expect(await call(undefined, 'nothing/here')).toEqual({ status: 404, body: { error: 'Unknown operation' } })
})

it("decodes path parameters and parses each route's query", async () => {
  const seen: unknown[] = []
  const call = serve({
    store: {
      settings: () => SettingsSchema.parse({}),
      read: {
        decisions: (filter: unknown, offset: number, limit: number) => {
          seen.push({ filter, offset, limit })
          return { items: [], more: false }
        },
      },
    },
    links: () => () => ({ url: '', label: { zh: '', en: '' } }),
    engine: {
      unsubscribe: (key: string) => {
        seen.push(key)
        return true
      },
    },
    sessions: { delivery: async () => 'injected' },
  })
  expect(await call('POST', `threads/${encodeURIComponent('feishu:oc_1:om/root')}/unsubscribe`)).toMatchObject({
    status: 200,
    body: { context_delivery: 'injected' },
  })
  // Invalid offsets default to 0, and limits above the route maximum are clamped.
  expect(await call('GET', 'decisions?kind=accept%2Creopen&limit=500&offset=x&window=24h')).toEqual({
    status: 200,
    body: { items: [], next_offset: null },
  })
  expect(seen).toEqual([
    'feishu:oc_1:om/root',
    { filter: { kinds: ['accept', 'reopen'], since: expect.any(String) as unknown }, offset: 0, limit: 200 },
  ])
})

it('answers only a same-origin request from the panel on the loopback address', async () => {
  const call = serve({ sessions: { delivery: async () => 'injected' }, engine: { unsubscribe: () => true } })
  const post = (headers: Headers) => call('POST', 'threads/github%3A1/unsubscribe', undefined, headers)
  for (const headers of [
    { 'x-orchestrator-request': undefined },
    { host: 'evil.example:3082' },
    { host: '127.0.0.1:9999' },
    { origin: 'http://evil.example' },
    { host: 'localhost:3082', origin: 'http://127.0.0.1:3082' },
  ]) {
    expect(await post(headers), JSON.stringify(headers)).toEqual({
      status: 403,
      body: { error: 'Same-origin management request required' },
    })
  }
  expect(await post({ host: 'localhost:3082', origin: 'http://localhost:3082' })).toMatchObject({ status: 200 })
  // Management routes require a loopback Web server.
  expect(() => serve({}, '0.0.0.0')).toThrow('Orchestrator management requires a loopback Web server')
})

it('answers a prompt save that lost a race with 409, and a settings body over 128 KiB with 400', async () => {
  const saves: unknown[] = []
  const call = serve({
    machine: { promptMaxChars: { get: () => 100_000 } },
    store: { atomic: (work: () => unknown) => work(), blockRevision: () => 0 },
    prompts: { save: () => { throw new PromptConflict('Prompt version changed to 3; compare the latest version before saving') } },
    sessions: { notify: () => {}, delivery: async () => 'injected' },
    settings: { save: async (...args: unknown[]) => { saves.push(args); return { saved: true, revision: 1, restarting: false } } },
  })
  expect(await call('PUT', 'prompts', { config: {}, baseRevision: 2 }))
    .toEqual({ status: 409, body: { error: 'Prompt version changed to 3; compare the latest version before saving' } })
  const value = { prepare: 'x'.repeat(128 * 1024) }
  expect(await call('PUT', 'settings/executor', { value, revision: 0 })).toEqual({ status: 400,
    body: { error: `Request exceeds ${128 * 1024} bytes` } })
  expect(saves).toEqual([])
})


it('serves every management view and action through the loopback routes', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'superagent-http-'))
  const store = new Store(join(directory, 'state.sqlite'))
  const sources = new SourceRegistry(() => store.settings())
  sources.register(feishuKind)
  sources.register(githubKind)
  const prompts = new Prompts(store, () => 100_000, () => [], () => true)
  const deployment = testDeployment()
  const machine = testMachine({ stateDir: directory })
  const ticket = { id: 't1', title: 'Fix', decisionId: 'd', channels: [], undelivered: [], createdAt: '', updatedAt: '',
    mandate: { strength: 'explicit' as const, policyVersion: 'v', basis: { via: 'listed' as const }, actor: { source: 'feishu', id: 'ou_alice' }, scope: '' },
    origin: { triggerId: 'trigger', triggerIds: ['trigger'] }, assignments: [{ id: 'a1', state: 'pending' as const, createdAt: '' }], inputs: [] }
  const originalTicketGet = store.tickets.get.bind(store.tickets)
  store.tickets.get = (id: string) => id === 't1' ? ticket : originalTicketGet(id)
  const runtime = {
    store,
    sourceLinks: new TextCache(1_000),
    sources,
    prompts,
    sessions: {
      status: () => ({ revision: prompts.current().revision, notificationPending: false, sessions: [] }),
      notify: vi.fn(),
      delivery: async () => 'injected',
    },
    settings: {
      view: vi.fn(),
      block: vi.fn(),
      save: vi.fn(async () => ({ saved: true, revision: 1, restarting: false })),
      pause: vi.fn(async () => ({ paused: true, restarting: false })),
      sync: vi.fn(async () => ({ source: 'github', allowed: 0, added: [], removed: [] })),
      refreshLabels: vi.fn(async () => ({ refreshed: true })),
    },
    engine: {
      unsubscribe: vi.fn(() => true),
      reevaluate: vi.fn(() => true),
      dismiss: vi.fn(),
      retryTicket: vi.fn(),
      stopTicket: vi.fn(),
    },
    executor: { activeCount: () => 0 },
    health: {},
    machine,
    masterId: 'main',
    ready: Promise.resolve(),
    configured: () => true,
    deployment,
    links: () => (channel: never) => sources.link(channel, deployment),
    ticketEnvironment: () => [],
    checkSetup: vi.fn(),
    requestPass: vi.fn(),
    models: async () => ({ default: { provider: 'p', model: 'm1' }, groups: [], failures: [] }),
  }
  const call = serve(runtime)
  expect(await call('GET', 'state')).toMatchObject({ status: 200, body: { masterId: 'main', paused: false } })
  expect(await call('GET', 'overview')).toMatchObject({ status: 200 })
  expect(await call('GET', 'search?q=issue')).toMatchObject({ status: 200 })
  expect(await call('GET', 'search')).toMatchObject({ status: 200 })
  expect(await call('GET', 'sources')).toMatchObject({ status: 200 })
  expect(await call('POST', 'sources/github/labels')).toMatchObject({ status: 200, body: { refreshed: true } })
  expect(await call('GET', 'threads?filter=all&offset=x&limit=500')).toMatchObject({ status: 200 })
  expect(await call('GET', 'threads/missing?limit=500&before=b&around=a')).toEqual({ status: 404, body: { error: 'Thread not found' } })
  expect(await call('POST', 'threads/github%3A1/unsubscribe')).toMatchObject({ status: 200, body: { removed: true } })
  expect(await call('POST', 'triggers/t1/requeue')).toEqual({ status: 200, body: { queued: true } })
  expect(await call('GET', 'decisions?kind=accept&window=24h&offset=x&limit=500')).toMatchObject({ status: 200 })
  expect(await call('GET', 'decisions')).toMatchObject({ status: 200 })
  expect(await call('POST', 'decisions/d1/dismiss')).toEqual({ status: 200, body: { dismissed: true } })
  expect(await call('GET', 'tickets?state=ended&offset=x&limit=500')).toMatchObject({ status: 200 })
  expect(await call('GET', 'tickets/missing')).toEqual({ status: 404, body: { error: 'Ticket not found' } })
  expect(await call('GET', 'tickets/t1')).toMatchObject({ status: 200, body: { id: 't1', environment: [] } })
  expect(await call('POST', 'tickets/t1/retry')).toEqual({ status: 200, body: { retried: true } })
  expect(await call('POST', 'tickets/t1/stop')).toEqual({ status: 200, body: { stopped: true } })
  expect(await call('POST', 'executor/pause', { paused: true })).toMatchObject({ status: 200, body: { paused: true } })
  expect(await call('PUT', 'settings/executor', { value: {}, revision: 0 })).toMatchObject({ status: 200, body: { saved: true } })
  expect(await call('GET', 'policy/history')).toMatchObject({ status: 200 })
  expect(await call('POST', 'policy-sync/github')).toMatchObject({ status: 200, body: { policy: { source: 'github' } } })
  expect(await call('POST', 'setup/check')).toEqual({ status: 200, body: { restarting: true } })
  expect(await call('GET', 'models')).toMatchObject({ status: 200, body: { default: { provider: 'p' } } })
  expect(await call('GET', 'prompts?revision=0&offset=1')).toMatchObject({ status: 200, body: { masterId: 'main' } })
  expect(await call('PUT', 'prompts', { config: defaultPrompts(), baseRevision: prompts.current().revision }))
    .toMatchObject({ status: 200, body: { effect: 'next-request' } })
  expect(await call('POST', 'prompts/preview', defaultPrompts())).toMatchObject({ status: 200, body: { scope: expect.any(String) as unknown } })

  const unconfigured = serve({ ...runtime, configured: () => false, setupProblem: { detail: 'waiting' } })
  expect(await unconfigured('GET', 'state')).toMatchObject({ status: 200, body: { setup: { accounts: null, problem: { detail: 'waiting' } } } })
  expect(await unconfigured('GET', 'sources')).toMatchObject({ status: 200 })
  expect(await unconfigured('GET', 'tickets/t1')).toMatchObject({ status: 200, body: { environment: [] } })
  const feishuOnly = serve({ ...runtime, configured: () => false, setupProblem: undefined, feishuProblem: { detail: 'feishu' } })
  expect(await feishuOnly('GET', 'state')).toMatchObject({ status: 200, body: { setup: { problem: null, feishuProblem: { detail: 'feishu' } } } })
  expect(await feishuOnly('GET', 'overview')).toMatchObject({ status: 200 })
  expect(await feishuOnly('GET', 'sources')).toMatchObject({ status: 200 })
  const noFeishu = serve({ ...runtime, deployment: testDeployment({ feishu: undefined }) })
  expect(await noFeishu('GET', 'state')).toMatchObject({ status: 200, body: { setup: { accounts: { feishu: null } } } })

  try {
    expect(runtime.engine.retryTicket).toHaveBeenCalledWith('t1')
    expect(runtime.engine.stopTicket).toHaveBeenCalledWith('t1')
    expect(runtime.checkSetup).toHaveBeenCalled()
    expect(runtime.settings.refreshLabels).toHaveBeenCalledWith('github')
  } finally {
    store.db.close()
    rmSync(directory, { recursive: true, force: true })
  }
})
