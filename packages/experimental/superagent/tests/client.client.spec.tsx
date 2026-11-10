// @vitest-environment jsdom
import { act, useState, type ComponentType } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { ApiError, caller, usePoll, type DecisionRow } from '../src/client/api.ts'
import { apply } from '../src/client/plugin.tsx'
import { PanelProvider, usePanel, usePanelData } from '../src/client/provider.tsx'
import { DecisionLine, Earlier, ThreadLine, TicketLine, automatedRuns } from '../src/client/rows.tsx'
import { CheckSetupButton, ClampText, ConfirmButton, ExternalLink, Hint, KeyValues, LabelValue, LoadMore, LoadState, MarkdownLite, PageHead, Section, SessionButton, SetupMessage, StageMark, StatusHead, When } from '../src/client/widgets.tsx'
import { zh } from '../src/client/locales.ts'
import { SettingsSchema } from '../src/policy.ts'
import { MEMBER, problemOf, threadSummary, ticketSummary } from './fixtures/panel-server.client.ts'

declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true

type Entry = {
  options: { name: string; id?: string; key?: string; order?: number; locale?: string; label?: () => string }
  component: ComponentType<Record<string, unknown>>
}
const t = ((key: keyof typeof zh) => zh[key] ?? key) as never
function fixture(listed: string[]) {
  const entries: Entry[] = []
  const dictionaries: Record<string, unknown> = {}
  const ctx = {
    slots: {
      inject: (_name: string, register: () => void) => {
        register()
      },
      register: (options: Entry['options'], component: Entry['component']) => {
        entries.push({ options, component })
        return () => {}
      },
    },
    locale: {
      register: (ns: string, value: unknown) => {
        dictionaries[ns] = value
        return () => {}
      },
      bind: () => t,
    },
    effect: (effect: () => unknown) => {
      effect()
    },
    sessions: {
      list: { getSnapshot: () => ({ phase: 'ready', byId: Object.fromEntries(listed.map(id => [id, {}])) }) },
      refresh: vi.fn(async () => {}),
    },
    uiWorkspace: { openSession: vi.fn() },
    // Omit config form service so Deployment renders read-only.
    get: () => undefined,
  }
  apply(ctx as never)
  const entry = (name: string) => entries.find(item => item.options.name === name)!
  return { ctx, entries, dictionaries, main: entry('main'), nav: entry('sidebar.panellist'), config: entries[2]! }
}
async function render(entry: Entry, props: Record<string, unknown>) {
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<entry.component {...props} />) })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  return { container, dispose: () => { act(() => { root.unmount() }); container.remove() } }
}
const openMaster = async (container: HTMLElement) => {
  const button = [...container.querySelectorAll('button')].find(item => item.textContent === zh.openMaster)!
  await act(async () => { button.click() })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
}
const serve = () => vi.stubGlobal('fetch', vi.fn(async (url: string) => {
  const path = new URL(url, 'http://127.0.0.1').pathname.replace('/orchestrator/api/', '')
  if (path === 'overview') return Response.json({ problems: [], person: [], executor: { running: 0, queued: 0, maxWorkers: 3, idle: 0 }, flow: { triggers: 0, filtered: 0, judged: 0, accepted: 0, evaluating: 0 }, policyVersion: 'v' })
  if (path === 'state') return Response.json({ masterId: 'master', settings: SettingsSchema.parse({}), revisions: { policy: 0, eval: 0, executor: 0 }, sources: [], policy_version: 'v', health: {}, outgoing: [], load: { running: 0, queued: 0 },
    setup: { machine: { stateDir: '/srv/state', worktreeRoot: '/srv/state/worktrees', blockedCommands: [], promptMaxChars: 100000, admissionContextMaxPages: 10, admissionContextTimeoutMs: 15000 }, problem: null, feishuProblem: null, accounts: null } })
  return Response.json({ error: 'unavailable' }, { status: 503 })
}))
afterEach(() => { vi.unstubAllGlobals() })

it('registers a localized main panel, sidebar entry and Plugin Manager row page, and no settings section', () => {
  const { entries, dictionaries, main, nav } = fixture([])
  expect(entries.map(entry => entry.options.name)).toEqual(['main', 'sidebar.panellist', 'plugins.row.config'])
  expect(entries[2]!.options).toMatchObject({ key: 'dsh-superagent#dsh-superagent', locale: 'dshSuperagent' })
  expect(Object.keys(dictionaries)).toEqual(['dshSuperagent'])
  expect(main.options).toMatchObject({ key: 'dsh-superagent', locale: 'dshSuperagent' })
  expect(nav.options).toMatchObject({ id: 'dsh-superagent', order: 10, locale: 'dshSuperagent' })
  expect(nav.options.label?.()).toBe('Superagent')
})

it('opens a listed Session through the workspace from the main panel', async () => {
  serve()
  const { ctx, main } = fixture(['master'])
  const view = await render(main, { t })
  await openMaster(view.container)
  expect(ctx.uiWorkspace.openSession).toHaveBeenCalledExactlyOnceWith('master')
  view.dispose()
})

it('reports a Session missing from the catalog in the interface language', async () => {
  serve()
  const { ctx, main } = fixture([])
  const view = await render(main, { t })
  await openMaster(view.container)
  expect(ctx.uiWorkspace.openSession).not.toHaveBeenCalled()
  expect(document.body.textContent).toContain(zh.sessionNotListed)
  view.dispose()
})

it('reports a Session list that is still loading in the interface language', async () => {
  serve()
  const { ctx, main } = fixture([])
  vi.spyOn(ctx.sessions.list, 'getSnapshot').mockReturnValue({ phase: 'loading', byId: {} })
  const view = await render(main, { t })
  await openMaster(view.container)
  expect(ctx.uiWorkspace.openSession).not.toHaveBeenCalled()
  expect(document.body.textContent).toContain(zh.sessionListNotReady)
  view.dispose()
})

it('renders the sidebar icon', async () => {
  const { nav } = fixture([])
  const view = await render(nav, { size: 24 })
  expect(view.container.querySelector('svg')?.getAttribute('viewBox')).toBe('0 0 24 24')
  view.dispose()
})

it('renders the Plugin Manager config page and omits its summary row', async () => {
  serve()
  const { config } = fixture([])
  const summary = await render(config, { t, view: 'summary', form: undefined })
  expect(summary.container.textContent).toBe('')
  summary.dispose()
  const page = await render(config, { t, view: 'page', form: undefined })
  expect(page.container.textContent).toContain(zh.deploymentIntro)
  page.dispose()
})

it('polls one request at a time, keeps the data while new dependencies load, and polls less while hidden', async () => {
  vi.useFakeTimers()
  const calls: { limit: number; done: (value: string[]) => void }[] = []
  let shown: { data?: string[]; limit: number } = { limit: 0 }
  let setLimit = (_limit: number) => {}
  function Harness() {
    const [limit, update] = useState(1)
    setLimit = update
    const { data } = usePoll(() => new Promise<string[]>((done) => { calls.push({ limit, done }) }), [limit], 1000)
    shown = { ...data ? { data } : {}, limit }
    return null
  }
  const container = document.createElement('div')
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  expect(calls).toHaveLength(1)
  // A slow response holds the next poll back instead of overlapping.
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect(calls).toHaveLength(1)
  await act(async () => { calls[0]!.done(['a']) })
  expect(shown.data).toEqual(['a'])
  await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
  expect(calls).toHaveLength(2)
  await act(async () => { setLimit(2) })
  expect(shown).toEqual({ data: ['a'], limit: 2 })
  await act(async () => { calls[2]!.done(['a', 'b']) })
  expect(shown.data).toEqual(['a', 'b'])
  // A late response to the superseded request must not replace the newer result.
  await act(async () => { calls[1]!.done(['stale']) })
  expect(shown.data).toEqual(['a', 'b'])
  const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
  await act(async () => { await vi.advanceTimersByTimeAsync(1000) })
  await act(async () => { calls[3]!.done(['a', 'b']) })
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) })
  expect(calls).toHaveLength(4)
  await act(async () => { document.dispatchEvent(new Event('visibilitychange')) })
  expect(calls).toHaveLength(4)
  hidden.mockRestore()
  await act(async () => { document.dispatchEvent(new Event('visibilitychange')) })
  expect(calls).toHaveLength(5)
  act(() => { root.unmount() })
  vi.useRealTimers()
})

it('wraps a rejected value that is not an Error so the panel can show it', async () => {
  let failure: Error | undefined
  function Harness() {
    const { error } = usePoll(vi.fn(async () => { throw 'offline' }), [])
    failure = error
    return null
  }
  const container = document.createElement('div')
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  await act(async () => { await Promise.resolve() })
  expect(failure).toEqual(new Error('offline'))
  act(() => { root.unmount() })
  container.remove()
})

it('ignores a poll failure that settles after the page unmounts', async () => {
  let fail: ((reason: string) => void) | undefined
  function Harness() {
    usePoll(() => new Promise((_done, reject) => { fail = reject }), [])
    return null
  }
  const container = document.createElement('div')
  const root = createRoot(container)
  await act(async () => { root.render(<Harness />) })
  act(() => { root.unmount() })
  await act(async () => { fail?.('late') })
  container.remove()
})

it('tells a dsh-superagent that does not answer from a record it does not have, and words each failure by what failed', async () => {
  const messages = { offline: 'offline', notRunning: 'not running', serverError: (status: number) => `server ${status}`,
    requestFailed: (status: number) => `request ${status}`, setup: () => 'setup', user: ({ code }: { code: string }) => `user ${code}` }
  const call = caller(() => messages)
  const failure = async (answer: () => Promise<Response>) => {
    vi.stubGlobal('fetch', vi.fn(answer))
    const error = await call('GET tickets/:id', { id: 't1' }).then(() => undefined, (thrown: unknown) => thrown)
    expect(error).toBeInstanceOf(ApiError)
    const typed = error as ApiError
    return { message: typed.message, status: typed.status, missing: typed.missing,
      unreachable: !!typed.details.unreachable, issues: typed.issues }
  }
  expect(await failure(async () => { throw new TypeError('Failed to fetch') })).toEqual({ message: 'offline', status: 0, missing: false,
    unreachable: true, issues: [] })
  // DSH's web server answers an empty 404 while dsh-superagent's routes are unregistered.
  expect(await failure(async () => new Response(null, { status: 404 }))).toEqual({ message: 'not running', status: 404, missing: false,
    unreachable: true, issues: [] })
  expect(await failure(async () => new Response(null, { status: 502 }))).toEqual({ message: 'server 502', status: 502, missing: false,
    unreachable: true, issues: [] })
  expect(await failure(async () => new Response('<html>Bad request</html>', { status: 400 }))).toEqual({ message: 'request 400',
    status: 400, missing: false, unreachable: false, issues: [] })
  expect(await failure(async () => Response.json({ issues: [{ path: 'title', message: 'empty' }] }, { status: 400 }))).toEqual({
    message: 'request 400', status: 400, missing: false, unreachable: false, issues: [{ path: 'title', message: 'empty' }] })
  expect(await failure(async () => Response.json({ error: 'Unknown ticket', code: 'unknownTicket' }, { status: 404 }))).toEqual({
    message: 'user unknownTicket', status: 404, missing: true, unreachable: false, issues: [] })
})


it('renders the shared widgets, confirms actions, opens Sessions, and rechecks setup', async () => {
  serve()
  const base = fetch
  const requests: string[] = []
  vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = new URL(input instanceof URL ? input.href : input, 'http://127.0.0.1').pathname
    requests.push(path)
    if (path.endsWith('setup/check')) return Response.json({})
    return base(input, init)
  }))
  const opened: string[] = [], confirmed: string[] = [], routes: unknown[] = []
  let backedUp = false
  let loadedMore = false
  function Suite() {
    return <PanelProvider t={t} navigate={(route) => { routes.push(route) }} back={() => { backedUp = true }} backLabel="Back"
      openSession={async (id) => {
        if (id === 'missing') throw new Error('Session disappeared')
        opened.push(id)
      }}>
      <When at="2026-11-09T12:00:00Z" />
      <SetupMessage problem={problemOf('repositoryUnset', {})} />
      <SetupMessage lead problem={problemOf('repositoryUnset', { output: 'gh auth status' })} />
      <Section title="Shared" count={2} hint="What is shared?" badge={<span>badge</span>} icon="settings"
        action={<button type="button">section action</button>}><Hint>section body</Hint></Section>
      <LabelValue label="model" value="deepseek" />
      <KeyValues rows={[['provider', 'deepseek'], ['mode', 'auto']]} />
      <MarkdownLite text={'# Title\n\nA **bold** `code` [link](https://example.com) paragraph.\n\n1. one\n2. two\n'} />
      <ClampText text={`long ${'x'.repeat(480)}`} />
      <ConfirmButton label="Confirm action" title="Confirm title" message="Confirm message"
        onConfirm={async () => { confirmed.push('action') }} variant="outline" icon="refresh" />
      <SessionButton sessionId="session-1" label="Open worker" />
      <SessionButton sessionId="missing" />
      <ExternalLink href="https://example.com">External</ExternalLink>
      <StageMark state="ongoing" icon="clock" label="running" />
      <StageMark state="done" icon="clock" label="done" />
      <LoadState loading />
      <LoadState error={new Error('load failed')} />
      <LoadMore onMore={() => { loadedMore = true }} />
      <StatusHead hint="Status" settings="source" />
      <PageHead title="Page">page action</PageHead>
      <CheckSetupButton />
    </PanelProvider>
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Suite />) })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(container.querySelector('ol')).not.toBeNull()
  expect(container.textContent).toContain('section body')
  const expand = [...container.querySelectorAll('button')].find(button => button.textContent === zh.expand)!
  await act(async () => { expand.click() })
  expect(expand.textContent).toBe(zh.collapse)
  const confirm = [...container.querySelectorAll('button')].find(button => button.textContent === 'Confirm action')!
  await act(async () => { confirm.click() })
  const accept = [...document.body.querySelectorAll('button')].find(button => button.textContent === zh.confirm)!
  await act(async () => { accept.click() })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(confirmed).toEqual(['action'])
  await act(async () => { [...container.querySelectorAll('button')].find(button => button.textContent === 'Open worker')!.click() })
  await act(async () => { [...container.querySelectorAll('button')].find(button => button.textContent === zh.openSession)!.click() })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(opened).toEqual(['session-1'])
  expect(document.body.textContent).toContain('Session disappeared')
  const settings = [...container.querySelectorAll('button')].find(button => button.textContent === zh['tab.settings'])!
  await act(async () => { settings.click() })
  const back = [...container.querySelectorAll('button')].find(button => button.textContent === 'Back')!
  await act(async () => { back.click() })
  const more = [...container.querySelectorAll('button')].find(button => button.textContent === zh.loadMore)!
  await act(async () => { more.click() })
  const check = [...container.querySelectorAll('button')].find(button => button.textContent === zh.setupCheck)!
  await act(async () => { check.click() })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(requests).toContain('/orchestrator/api/setup/check')
  expect(backedUp).toBe(true); expect(loadedMore).toBe(true); expect(routes[0]).toMatchObject({ page: 'settings' })
  act(() => { root.unmount() }); container.remove()
})

it('refuses panel hooks outside the panel provider', async () => {
  function OutsideData() { usePanelData(); return null }
  function OutsidePanel() { usePanel(); return null }
  for (const Outside of [OutsideData, OutsidePanel]) {
    const container = document.createElement('div'); document.body.append(container)
    const root = createRoot(container)
    expect(() => { act(() => { root.render(<Outside />) }) }).toThrow('dsh-superagent page rendered outside its panel')
    act(() => { root.unmount() }); container.remove()
  }
})

it('renders ticket, thread, decision, and earlier rows with their navigation and folding behavior', async () => {
  serve()
  const routes: unknown[] = []
  const person = ticketSummary({
    note: { text: '[image] answer needed', error: true },
    reply: { url: 'https://github.com/acme/widgets/issues/1', label: { zh: 'GitHub', en: 'GitHub' } },
    actions: { retry: true, stop: true }, sessionId: 'worker-t1', undelivered: 2,
  }, { stage: 'person', action: 'answer' })
  const plain = ticketSummary({ note: undefined, reply: undefined, actions: { retry: false, stop: false }, sessionId: '' })
  const subscribed = threadSummary({
    tickets: [{ id: 't1', title: 'First', state: 'open' }, { id: 't2', title: 'Second', state: 'open' }],
    latest: { actor: { source: 'feishu', id: MEMBER, name: { zh: 'Alice', en: 'Alice' } }, human: true,
      at: '2026-11-09T12:00:00Z', preview: 'latest message' },
  })
  const failed = threadSummary({ tickets: [], latest: undefined, pollError: 'poll failed', url: undefined, facts: {} })
  const decision = (fields: Partial<DecisionRow>): DecisionRow => ({
    id: 'd1', kind: 'accept', outcome: 'accept', reason: [{ code: 'said', values: { text: 'needed' } }],
    at: '2026-11-09T12:00:00Z', strength: 'explicit', ticketId: 't1', triggerIds: ['d1'], thread: 'feishu:oc_1:om_root',
    rules: [{ name: 'rule', matched: true, reason: 'matched' }], threadTitle: 'Thread title', preview: 'decision preview',
    ticketTitle: 'Decision ticket', ...fields,
  })
  function Suite() {
    return <PanelProvider t={t} navigate={(route) => { routes.push(route) }}>
      <ul>
        <TicketLine ticket={person} question context onChanged={async () => {}} />
        <TicketLine ticket={plain} />
        <ThreadLine thread={subscribed} ticketId="t1" context />
        <ThreadLine thread={failed} />
        <DecisionLine decision={decision({})} />
        <DecisionLine decision={decision({ ticketId: undefined, ticketTitle: undefined, threadTitle: 'Trigger thread' })} />
      </ul>
      <Earlier items={[]} />
      <Earlier items={[{ id: 'e1', at: '2026-11-09T12:00:00Z', preview: '[image]' }, { id: 'e2', at: '2026-11-09T11:00:00Z', preview: '' }]} />
    </PanelProvider>
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  await act(async () => { root.render(<Suite />) })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(container.textContent).toContain('answer needed')
  expect(container.textContent).toContain('poll failed')
  expect(container.textContent).toContain('decision preview')
  const ticket = [...container.querySelectorAll('button')].find(button => button.textContent === 'Decision ticket')!
  await act(async () => { ticket.click() })
  const trigger = [...container.querySelectorAll('button')].find(button => button.textContent === 'Trigger thread')!
  await act(async () => { trigger.click() })
  expect(automatedRuns([{ id: '1', sender: 'bot' }, { id: '2', sender: 'bot' }, { id: '3', sender: 'person' }, { id: '4' }],
    item => item.sender)).toEqual([
    { item: { id: '1', sender: 'bot' }, earlier: [{ id: '2', sender: 'bot' }] },
    { item: { id: '3', sender: 'person' }, earlier: [] },
    { item: { id: '4' }, earlier: [] },
  ])
  const earlier = [...container.querySelectorAll('button')].find(button => button.textContent?.startsWith(zh.automatedEarlier.split('{')[0] ?? ''))!
  await act(async () => { earlier.click() })
  expect(container.textContent).toContain(zh.imageMarker)
  expect(container.textContent).toContain(zh.empty)
  expect(routes).toEqual([
    { page: 'execution', id: 't1' },
    { page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: 'd1' } },
  ])
  act(() => { root.unmount() }); container.remove()
})
