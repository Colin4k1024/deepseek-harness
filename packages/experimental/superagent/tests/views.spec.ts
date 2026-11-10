import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { z } from 'zod'
import { Store } from '../src/app/store.ts'
import { Commands } from '../src/infra/commands.ts'
import { testDeployment } from './fixtures/deployment.ts'
import { SettingsSchema } from '../src/policy.ts'
import { SourceRegistry } from '../src/sources/registry.ts'
import { feishuKind } from '../src/sources/feishu.ts'
import type { SourceKind } from '../src/sources/kind.ts'
import { githubKind } from '../src/sources/github.ts'
import { awaitingEval, channelLink, decisionView, page, snapshot, threadSummary, ticketSummary, triggerView } from '../src/app/views/records.ts'
import { basisFacts, executorLoad, ticketDetail, ticketList } from '../src/app/views/tickets.ts'
import { decisionList, search, threadDetail, threadList } from '../src/app/views/threads.ts'
import { droppedReplies, sourcesStatus } from '../src/app/views/sources.ts'
import { overview } from '../src/app/views/overview.ts'
import { markdown, plain } from '../src/app/views/text.ts'
import { policyHistory } from '../src/app/settings-history.ts'
import { settingsView } from '../src/app/views/settings.ts'
import { KV } from '../src/kv.ts'
import { feishuLink, sendFeishu } from '../src/sources/feishu-channel.ts'
import { feishuChannel, feishuTriggerId } from '../src/sources/feishu-trigger.ts'
import { githubLink } from '../src/sources/github-channel.ts'
import { githubChannel } from '../src/sources/github-trigger.ts'
import { followupOwners, type ChannelRef, type Decision } from '../src/core/index.ts'

let directory: string
let store: Store
beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), 'dsh-superagent-views-'))
  store = new Store(join(directory, 'orchestrator.sqlite'))
})
afterEach(() => { store.close(); rmSync(directory, { recursive: true, force: true }) })
const links = (channel: ChannelRef) => (channel.source === 'feishu' ? feishuLink : githubLink)(channel.address, {})
const drop = (id: string, channel: ChannelRef, ticketId?: string) => {
  const message = { id, channel, text: id, kind: 'reply' as const, ...ticketId ? { ticketId } : {} }
  store.receipts.add(message)
  store.receipts.settle(message, 'channelGone')
}

it('links a Feishu chat with its defaults and refuses a reply with no target message', async () => {
  expect(feishuLink({}, {})).toEqual({ url: 'https://applink.feishu.cn/client/chat/open?openChatId=', label: { zh: '飞书消息', en: 'Feishu message' } })
  const commands = new Commands(() => testDeployment(), {}, new AbortController().signal)
  await expect(sendFeishu({ commands }, { id: 'r', channel: { source: 'feishu', thread: 'feishu:oc_1', address: {} }, text: 'x', kind: 'reply' }))
    .rejects.toThrow('Missing Feishu reply target')
})

it('names the Thread of each dropped message: the one of the Trigger its Channel answers, or the Channel\'s own', () => {
  // A Feishu Channel names its chat; the message it answers belongs to the Thread of the topic's root message.
  store.triggers.save({ id: feishuTriggerId('om_reply'), source: 'feishu', threads: [{ source: 'feishu', id: 'oc_1:om_root' }],
    actor: { source: 'feishu', id: 'ou_a' }, human: true, at: new Date().toISOString(), text: 'please', meta: {} })
  store.threads.save({ key: 'github:42', ref: { source: 'github', id: '42' }, createdAt: '', subscriptions: [], correlated: [], pending: [],
    meta: {} })
  drop('a', feishuChannel('oc_1', 'om_reply'))
  drop('b', githubChannel(42))
  // A message whose Trigger and Thread the store no longer has links nowhere.
  drop('c', feishuChannel('oc_2', 'om_gone'))
  expect(Object.fromEntries(droppedReplies({ store, links }).map(item => [item.id, item.thread])))
    .toEqual({ a: 'feishu:oc_1:om_root', b: 'github:42', c: undefined })
})

function views() {
  store.saveSettings(SettingsSchema.parse({ policy: { sources: { feishu: { allowed: [{ id: 'ou_alice', name: 'Alice' }] },
    github: { allowed: [{ id: 'alice' }] } },
  rules: [{ id: 'copy', name: 'Copy fixes', enabled: true, instruction: 'Fix copy' }] } }))
  const sources = new SourceRegistry(() => store.settings())
  sources.register(feishuKind); sources.register(githubKind)
  return { store, sources, links }
}
const at = new Date().toISOString()
/** A live ticket with no Assignment, as a follow-up's owner needs one. */
const live = (id: string) =>{  store.tickets.save({ id, title: id, decisionId: 'd', channels: [], undelivered: [], createdAt: at,
  updatedAt: at,
  mandate: { strength: 'explicit', policyVersion: 'v', basis: { via: 'listed' }, actor: { source: 'feishu', id: 'ou_alice' }, scope: '' },
  origin: { triggerId: 'e0', triggerIds: ['e0'] }, assignments: [], inputs: [] }) }
const message = (id: string, actor: { source: string; id: string }, human = true, time = at) => ({ id, source: actor.source,
  threads: [{ source: 'feishu', id: 'oc_1:om_root' }], actor, human, at: time, text: `message ${id}`, meta: {} })

it('names each sender as the Policy lists it, by its id where the id is readable, and as its kind names one it does not list', () => {
  const view = snapshot(views())
  expect([view.actor({ source: 'feishu', id: 'OU_ALICE' }), view.actor({ source: 'github', id: 'alice' }), view.actor({ source: 'github',
    id: 'bob' }),
  view.actor({ source: 'feishu', id: 'ou_bob' }), view.actor({ source: 'feishu', id: 'ou_bot' }, false), view.actor({ source: 'feishu',
    id: '' })])
    .toEqual([{ source: 'feishu', id: 'OU_ALICE', name: { zh: 'Alice', en: 'Alice' } }, { source: 'github', id: 'alice',
      name: { zh: 'alice', en: 'alice' } },
    { source: 'github', id: 'bob', name: { zh: 'bob', en: 'bob' } }, { source: 'feishu', id: 'ou_bob',
      name: feishuKind.policy!.unnamed!.person },
    { source: 'feishu', id: 'ou_bot', name: feishuKind.policy!.unnamed!.bot }, { source: 'feishu', id: '' }])
  expect(view.rule('copy')).toEqual({ id: 'copy', name: 'Copy fixes' })
  expect(view.rule('gone')).toEqual({ id: 'gone', name: 'gone' })
})

it('classifies a Thread by what it is doing, and offers a filtered Trigger of a person to Eval again', () => {
  const host = views()
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: [],
    correlated: [],
    pending: ['e2'], meta: {} })
  store.triggers.save(message('e1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.save(message('e2', { source: 'feishu', id: 'ou_bob' }))
  store.triggers.save(message('e3', { source: 'feishu', id: 'ou_bot' }, false))
  for (const id of ['e1', 'e3']) store.triggers.saveDisposition(id, { filtered: { code: 'not_a_member' }, fanout: [], at })
  const view = snapshot(host)
  expect(threadSummary(view, store.threads.get('feishu:oc_1:om_root')!).stage).toBe('queued')
  const rows = ['e1', 'e2', 'e3'].map(id => triggerView(view, store.triggers.get(id)!))
  // A queued Trigger waits for Eval already, and an automated one is not offered.
  expect(rows.map(row => row.actions.requeue)).toEqual([true, false, false])
  expect(rows[0]!.actor.name).toEqual({ zh: 'Alice', en: 'Alice' })
})

it('states each Source by the first state that holds', () => {
  const host = views()
  const states = (setup: Parameters<typeof sourcesStatus>[2], health: Parameters<typeof sourcesStatus>[1] = {}) =>
    Object.fromEntries(sourcesStatus(host, health, setup).map(source => [source.id, source.state]))
  expect(states({})).toEqual({ feishu: 'starting', github: 'starting' })
  const sourceProblem = { place: 'github', code: 'agentCliUnset', values: {}, detail: '' } as const
  expect(states({ problem: sourceProblem })).toEqual({ feishu: 'setup', github: 'error' })
  expect(states({}, { feishu: { state: 'ready', detail: '' }, history: { state: 'ready', detail: '' }, github: { state: 'ready', detail: '' } }))
    .toEqual({ feishu: 'ready', github: 'ready' })
  expect(states({}, { feishu: { state: 'ready', detail: '' }, history: { state: 'paused', detail: '' }, github: { state: 'error',
    detail: 'x' } }))
    .toEqual({ feishu: 'paused', github: 'error' })
  const problem = { place: 'deployment', code: 'stateDirUnset', values: {}, detail: '' } as never
  expect(states({ problem }, { feishu: { state: 'ready', detail: '' }, history: { state: 'ready', detail: '' }, github: { state: 'ready',
    detail: '' } }))
    .toEqual({ feishu: 'setup', github: 'setup' })
  const now = new Date().toISOString()
  store.triggers.save(message('filtered-now', { source: 'feishu', id: 'ou_bob' }, true, now))
  store.triggers.saveDisposition('filtered-now', { filtered: { code: 'not_a_member' }, fanout: [], at: now })
  const older = new Date(Date.now() - 48 * 60 * 60 * 1000).toISOString()
  store.triggers.save(message('filtered-old', { source: 'feishu', id: 'ou_bob' }, true, older))
  store.triggers.saveDisposition('filtered-old', { filtered: { code: 'not_a_member' }, fanout: [], at: older })
  const rows = sourcesStatus(host, {}, {})
  expect(rows.find(source => source.id === 'feishu')).toMatchObject({
    stats: { received: 1, filtered: 1, lastAt: now },
    filtered: [{ id: 'filtered-now', at: now, actor: { source: 'feishu', id: 'ou_bob' }, preview: 'message filtered-now',
      reason: { code: 'not_a_member' } }],
  })
  expect(rows.find(source => source.id === 'github')).toMatchObject({ stats: { received: 0, filtered: 0, lastAt: '' }, filtered: [] })
})

it('follows the Ticket\'s stage: a blocked Ticket a person answered since is working, with no retry, note or turn end', () => {
  const host = views()
  const blocked = { state: 'blocked' as const, summary: 'Needs the design file', at: '2026-09-22T00:01:00Z' }
  const ticket = { id: 't1', title: 'Fix', decisionId: 'd', channels: [], undelivered: [], createdAt: at, updatedAt: at,
    mandate: { strength: 'explicit' as const, policyVersion: 'v', basis: { via: 'listed' as const }, actor: { source: 'feishu',
      id: 'ou_alice' }, scope: '' },
    origin: { triggerId: 'e1', triggerIds: ['e1'] }, assignments: [{ id: 'a1', state: 'running' as const, createdAt: at, startedAt: at,
      report: blocked }],
    inputs: [] as { id: string; kind: 'trigger'; triggerId: string; at: string; human?: true }[] }
  store.tickets.save(ticket)
  const row = ticketSummary(store.tickets.get('t1')!)
  expect(row).toMatchObject({ stage: 'person', action: 'blocked', actions: { retry: true }, note: { text: 'Needs the design file',
    error: false } })
  expect(ticketDetail(host, 't1')!.assignments[0]).toMatchObject({ turnEndedAt: blocked.at })
  store.tickets.save({ ...ticket, inputs: [{ id: 'i1', kind: 'trigger', triggerId: 'e2', at: '2026-09-22T00:02:00Z', human: true }] })
  const answered = ticketSummary(store.tickets.get('t1')!)
  expect(answered).toMatchObject({ stage: 'working', actions: { retry: false } })
  expect(answered).not.toHaveProperty('note')
  expect(ticketDetail(host, 't1')!.assignments[0]).not.toHaveProperty('turnEndedAt')
})

it('does not show a follow-up that reached a ticket as waiting for an Eval, though an older version queued it', () => {
  const host = views()
  live('t1')
  store.triggers.save(message('f1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f1', { fanout: ['t1'], at })
  store.triggers.save(message('f2', { source: 'feishu', id: 'ou_bob' }))
  store.triggers.saveDisposition('f2', { fanout: ['t1'], requeued: true, at })
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: ['t1'],
    correlated: [],
    pending: ['f1'], meta: {} })
  const view = snapshot(host)
  expect(threadSummary(view, store.threads.get('feishu:oc_1:om_root')!)).toMatchObject({ stage: 'subscribed', pending: 0 })
  expect(triggerView(view, store.triggers.get('f1')!).disposition.queued).toBe(false)
  // One a person sent to Eval again waits for it.
  store.threads.save({ ...store.threads.get('feishu:oc_1:om_root')!, pending: ['f1', 'f2'] })
  expect(threadSummary(snapshot(host), store.threads.get('feishu:oc_1:om_root')!)).toMatchObject({ stage: 'queued', pending: 1 })
})

const load = { running: 0, queued: 0 }
const problems = (host: ReturnType<typeof views>, health: Parameters<typeof overview>[1] = {}) =>
  overview(host, health, 'v1', load).problems

it('lists an Eval past its deadline, and as dsh-superagent\'s own errors only parts no Source or Policy sync reports', () => {
  const host = views()
  const evaluating = { id: 'e1', threadKey: 'github:1', triggerIds: ['t1'], strength: 'explicit' as const, actor: { source: 'github',
    id: 'alice' },
  basis: { via: 'addressed' as const, addressed: 'mention' as const }, policyVersion: 'v1', startedAt: '2026-01-01T00:00:00Z',
  deadline: '2026-01-01T00:02:00Z' }
  store.threads.save({ key: 'github:1', ref: { source: 'github', id: '1' }, createdAt: at, subscriptions: [], correlated: [], pending: [],
    evaluating, meta: {} })
  const error = (detail: string) => ({ state: 'error' as const, detail })
  const health = { relay: error('relay failing'), reply: error('reply failing'), feishu: error('socket closed'),
    history: error('read failed'),
    'sync:feishu': error('sync failed'), evals: error('unavailable'), launch: { state: 'ready' as const, detail: '' } }
  const found = problems(host, health)
  expect(found).toContainEqual({ kind: 'eval-overdue', thread: expect.objectContaining({ key: 'github:1', stage: 'evaluating' }) as unknown })
  // Effects are dsh-superagent's own; a Source shows its parts and its Policy sync, and the Evals are listed one by one.
  expect(found.filter(item => item.kind === 'runtime-error')).toEqual([{ kind: 'runtime-error', name: 'relay', detail: 'relay failing' },
    { kind: 'runtime-error', name: 'reply', detail: 'reply failing' }])
  expect(found.filter(item => item.kind === 'source-error').map(item => item.source.health.name)).toEqual(['feishu', 'history'])
})

it('says no Source runs when every Source is off, and states a Source missing or off', () => {
  const host = views()
  expect(problems(host).map(item => item.kind)).not.toContain('no-source')
  const settings = store.settings()
  store.saveSettings({ ...settings, sources: { feishu: { enabled: false, config: {} }, github: { enabled: false, config: {} },
    jira: { enabled: true, config: { site: 'acme' } } } })
  expect(problems(host).map(item => item.kind)).toContain('no-source')
  expect(Object.fromEntries(sourcesStatus(host, {}, {}).map(source => [source.id, [source.state, source.installed]])))
    .toEqual({ feishu: ['off', true], github: ['off', true], jira: ['missing', false] })
})

it('sends text without HTML, entity-encoded tags included, and with images named', () => {
  const text = '<div>Hello <b>there</b></div> &lt;script&gt;alert(1)&lt;/script&gt; <!-- note --> ![shot](https://x/y.png) [Image: diagram]'
  expect(plain(text)).toBe('Hello there alert(1) [image] [image]')
  expect(markdown(`**Fix** ${text}\n\n- one`)).toBe('**Fix** Hello there alert(1)  [image] [image]\n\n- one')
  expect(plain('[the docs](https://example.com) and `code` &amp; more')).toBe('the docs and code & more')
})

it('words each settings save by what it changed from the one before, and nothing after a gap in the history', () => {
  const host = views()
  const first = store.settings()
  const allowed = (entries: { id: string; name?: string }[]) => ({ ...first.policy, sources: { ...first.policy.sources,
    feishu: { ...first.policy.sources.feishu!, allowed: entries } } })
  const alice = { id: 'ou_alice', name: 'Alice Liu' }, bob = { id: 'ou_bob', name: 'Bob' }
  store.saveSettings({ ...first, policy: allowed([alice, bob, { id: 'ou_carol' }]), paused: true,
    sources: { ...first.sources, feishu: { enabled: false, config: {} } },
    executor: { ...first.executor, directory: '/srv/work', prepare: 'npm ci', blockedCommands: ['rm', 'curl'] } })
  const second = store.settings()
  store.saveSettings({ ...second, policy: allowed([alice, { id: 'ou_carol' }]), paused: false, sources: { ...second.sources,
    feishu: { enabled: true, config: {} } } })
  const changes = (revision: number) => policyHistory(store, host.sources, 'v').items.find(item => item.revision === revision)!.changes
  const revision = store.get(KV.settings.revision)!
  expect(changes(revision - 1)).toEqual([
    { code: 'sourceRowsAdded', source: 'feishu', part: true, field: 'allowed', items: ['Bob', 'ou_carol'] },
    { code: 'sourceRowsChanged', source: 'feishu', part: true, field: 'allowed', items: ['Alice Liu'] },
    { code: 'paused' }, { code: 'executionDirectory', from: '', to: '/srv/work' }, { code: 'prepareCommand' },
    { code: 'blockedCommands', from: '', to: 'rm curl' }, { code: 'sourceDisabled', source: 'feishu' }])
  expect(changes(revision)).toEqual([{ code: 'sourceRowsRemoved', source: 'feishu', part: true, field: 'allowed', items: ['Bob'] },
    { code: 'resumed' }, { code: 'sourceEnabled', source: 'feishu' }])
  // A save whose predecessor is missing from the history has nothing to compare with.
  store.delete(KV.settings.version(revision))
  store.saveSettings({ ...store.settings(), paused: true })
  expect(changes(revision + 1)).toBeNull()
})

it('words every kind of settings change, including rows without labels and Sources this build does not know', () => {
  const host = views()
  const config = {
    feishu: { cli: 'lark-cli', chats: [], directMessages: true, otherChats: 'none', readPageSize: 20, linkTimeoutMs: 5000 },
    github: { repository: 'acme/widgets', cli: 'gh', pollSeconds: 60, failuresBeforeError: 3 },
  }
  const rule = (id: string, name: string, enabled: boolean, instruction = `${id} instruction`) => ({ id, name, enabled, instruction })
  const before = SettingsSchema.parse({
    paused: false,
    sources: { ...config, feishu: { enabled: true, config: config.feishu }, github: { enabled: true, config: config.github },
      jira: { enabled: true, config: { site: 'old' } }, old: { enabled: true, config: { site: 'old' } } },
    policy: {
      rules: [rule('copy', 'Copy fixes', true), rule('visual', 'Visual fixes', true), rule('docs', 'Docs fixes', false)],
      sources: {
        feishu: { allowed: [{ id: 'ou_alice', name: 'Alice' }, { id: 'ou_keep', name: 'Keep' }], syncChat: 'oc_old', bots: [{ id: 'cli_old' }] },
        github: { allowed: [{ id: 'alice' }, { id: 'keep' }], bots: [{ id: 'old-bot[bot]' }] },
        jira: { allowed: [{ id: 'jira-old' }], admission: { person: { listedUnaddressed: 'ignore' } } },
        old: { allowed: [{ id: 'old-user' }], admission: { person: { unlistedAddressed: 'explicit' } } },
      },
    },
    eval: { mode: 'observe', timeoutSeconds: 120, model: null },
    executor: { model: null, maxWorkers: 3, directory: '', prepare: '', blockedCommands: [] },
  })
  const after = SettingsSchema.parse({
    paused: true,
    sources: { feishu: { enabled: true, config: { ...config.feishu, directMessages: false, readPageSize: 30 } },
      github: { enabled: true, config: { ...config.github, pollSeconds: 90, failuresBeforeError: 4, observeIssues: true } },
      jira: { enabled: false, config: { site: 'new' } }, slack: { enabled: false, config: {} } },
    policy: {
      rules: [rule('copy', 'Copy changes', false, 'copy changed instruction'), rule('docs', 'Docs fixes', true), rule('other', 'Other fixes', true)],
      sources: {
        feishu: { allowed: [{ id: 'ou_bob' }, { id: 'ou_keep' }], syncChat: 'oc_new', bots: [{ id: 'cli_new' }] },
        github: { allowed: [{ id: 'bob' }, { id: 'keep' }], bots: [{ id: 'new-bot[bot]' }] },
        jira: { allowed: [{ id: 'jira-new' }], admission: { person: { listedUnaddressed: 'inferred' } } },
        slack: { allowed: [{ id: 'slack-user' }], admission: { person: { unlistedAddressed: 'explicit' } } },
      },
    },
    eval: { mode: 'auto', timeoutSeconds: 240, model: { provider: 'provider', model: 'model' } },
    executor: { model: { provider: 'worker-provider', model: 'worker-model' }, maxWorkers: 5, directory: '/srv/work',
      prepare: 'npm ci', blockedCommands: ['rm'] },
  })
  const record = { source: 'panel' as const, policyVersion: 'v', policyChanged: true, notified: ['missing'] }
  store.saveSettings(before, record)
  store.saveSettings(after, record)
  const revision = store.get(KV.settings.revision)!
  const history = policyHistory(store, host.sources, 'v')
  expect(history.items[0]).toMatchObject({ revision, notified: [{ id: 'missing', title: 'missing' }] })
  expect(history.items[0]!.changes).toEqual(expect.arrayContaining([
    { code: 'ruleRenamed', from: 'Copy fixes', to: 'Copy changes' },
    { code: 'ruleInstruction', name: 'Copy changes' },
    { code: 'ruleDisabled', name: 'Copy changes' },
    { code: 'ruleEnabled', name: 'Docs fixes' },
    { code: 'ruleAdded', name: 'Other fixes' },
    { code: 'ruleRemoved', name: 'Visual fixes' },
    { code: 'sourceRowsAdded', source: 'feishu', part: true, field: 'allowed', items: ['ou_bob'] },
    { code: 'sourceRowsRemoved', source: 'feishu', part: true, field: 'allowed', items: ['Alice'] },
    { code: 'sourceRowsChanged', source: 'feishu', part: true, field: 'allowed', items: ['ou_keep'] },
    { code: 'sourceField', source: 'feishu', part: true, field: 'syncChat', from: 'oc_old', to: 'oc_new' },
    { code: 'sourceRowsAdded', source: 'feishu', part: true, field: 'bots', items: ['cli_new'] },
    { code: 'sourceRowsRemoved', source: 'feishu', part: true, field: 'bots', items: ['cli_old'] },
    { code: 'sourceRowsAdded', source: 'github', part: true, field: 'allowed', items: ['bob'] },
    { code: 'sourceRowsRemoved', source: 'github', part: true, field: 'allowed', items: ['alice'] },
    { code: 'sourceRowsAdded', source: 'github', part: true, field: 'bots', items: ['new-bot[bot]'] },
    { code: 'sourceRowsRemoved', source: 'github', part: true, field: 'bots', items: ['old-bot[bot]'] },
    { code: 'sourceAdmission', source: 'jira', part: true },
    { code: 'sourceAdmission', source: 'old', part: true },
    { code: 'sourceAdmission', source: 'slack', part: true },
    { code: 'mode', from: 'observe', to: 'auto' },
    { code: 'timeout', from: 120, to: 240 },
    { code: 'evalModel' }, { code: 'executorModel' }, { code: 'maxWorkers', from: 3, to: 5 },
    { code: 'paused' }, { code: 'executionDirectory', from: '', to: '/srv/work' }, { code: 'prepareCommand' },
    { code: 'blockedCommands', from: '', to: 'rm' },
    { code: 'sourceField', source: 'feishu', field: 'directMessages', from: true, to: false },
    { code: 'sourceField', source: 'feishu', field: 'readPageSize', from: 20, to: 30 },
    { code: 'sourceField', source: 'github', field: 'pollSeconds', from: 60, to: 90 },
    { code: 'sourceField', source: 'github', field: 'failuresBeforeError', from: 3, to: 4 },
    { code: 'sourceField', source: 'github', field: 'observeIssues', from: false, to: true },
    { code: 'sourceDisabled', source: 'jira' }, { code: 'sourceDisabled', source: 'slack' },
  ]))
  const current = store.get(KV.settings.version(revision))!
  store.set(KV.settings.version(revision + 1), { ...current, revision: revision + 1 })
  store.set(KV.settings.revision, revision + 1)
  store.delete(KV.settings.version(revision))
  store.delete(KV.settings.revision)
  const gapped = policyHistory(store, host.sources, 'v')
  expect(gapped.current).toBe(0)
  expect(gapped.items[0]!.changes).toBeNull()
})

it('names a listed bot as the Policy lists it, and gives a bot\'s Mandate a basis fact of its own', () => {
  store.saveSettings(SettingsSchema.parse({ policy: { sources: { feishu: { allowed: [], bots: [{ id: 'cli_review', name: '评审机器人' }] } } } }))
  const sources = new SourceRegistry(() => store.settings())
  sources.register(feishuKind)
  const view = snapshot({ store, sources, links })
  expect(view.actor({ source: 'feishu', id: 'cli_review' }, false).name).toEqual({ zh: '评审机器人', en: '评审机器人' })
  expect(basisFacts({ via: 'bot', sender: '评审机器人', addressed: 'mention', observation: { mode: 'auto' } }, id => view.rule(id)).facts)
    .toEqual([{ name: 'sender', value: '评审机器人' }, { name: 'bot' }, { name: 'addressed', value: 'mention' }, { name: 'mode', value: 'auto' }])
})

it('shows a follow-up that reached a ticket as delivered, the tickets that handed a Trigger back with why, and the Threads a ticket follows for context', () => {
  const host = views()
  live('t1')
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: ['t1'],
    correlated: [],
    pending: [], meta: {} })
  store.triggers.save(message('f1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f1', { fanout: ['t1'], at })
  store.triggers.save(message('f2', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f2', { fanout: ['t1'], requeued: true, handbacks: [{ ticketId: 't1', reason: 'Asks for a new feature', at }], at })
  const view = snapshot(host)
  expect(triggerView(view, store.triggers.get('f1')!).disposition).toMatchObject({ delivered: true, queued: false, handbacks: [] })
  // A Trigger handed back goes to Eval as any message on its Thread; the Ticket that handed it back is named by its title once it has one.
  expect(triggerView(view, store.triggers.get('f2')!).disposition).toMatchObject({ delivered: false,
    handbacks: [{ ticketId: 't1', title: 't1', reason: 'Asks for a new feature' }] })
  store.threads.save({ key: 'github:7', ref: { source: 'github', id: '7' }, createdAt: at, subscriptions: ['t1'], context: ['t1'],
    correlated: [], pending: [], meta: {} })
  expect(threadDetail(host, 'github:7', undefined, 50)?.context).toEqual(['t1'])
  store.tickets.save({ id: 't1', title: 'Fix', decisionId: 'd', channels: [], undelivered: [], createdAt: at, updatedAt: at,
    mandate: { strength: 'explicit', policyVersion: 'v', basis: { via: 'listed' }, actor: { source: 'feishu', id: 'ou_alice' }, scope: '' },
    origin: { triggerId: 'e1', triggerIds: ['e1'] }, assignments: [],
    inputs: [{ id: 'i1', kind: 'trigger', triggerId: 'f1', at }, { id: 'i2', kind: 'trigger', triggerId: 'f2', at }] })
  const page = ticketDetail(host, 't1')!
  expect(Object.fromEntries(page.threads.map(thread => [thread.key, !!thread.context]))).toEqual({ 'feishu:oc_1:om_root': false,
    'github:7': true })
  expect(page.inputs.map(input => input.kind === 'trigger' && input.trigger?.handback)).toEqual([{ reason: 'Asks for a new feature', at },
    undefined])
  expect(triggerView(snapshot(host), store.triggers.get('f2')!).disposition.handbacks[0]?.title).toBe('Fix')
})

it('names a bot\'s Mandate as a bot once the Policy no longer lists it', () => {
  const host = views()
  const mandate = { strength: 'inferred' as const, policyVersion: 'v', basis: { via: 'bot' as const,
    observation: { mode: 'auto' as const } },
  actor: { source: 'feishu', id: 'cli_removed' }, scope: '' }
  store.tickets.save({ id: 't1', title: 'Fix', decisionId: 'd', channels: [], undelivered: [], createdAt: at, updatedAt: at, mandate,
    origin: { triggerId: 'e1', triggerIds: ['e1'] }, assignments: [], inputs: [] })
  expect(ticketDetail(host, 't1')!.mandate.actor).toEqual({ source: 'feishu', id: 'cli_removed', name: feishuKind.policy!.unnamed!.bot })
})

it('offers a person to send a follow-up to Eval while a ticket decides it, or once nothing did', () => {
  const host = views()
  live('t1')
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: ['t1'],
    correlated: [],
    pending: [], meta: {} })
  store.triggers.save(message('f1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f1', { fanout: ['t1'], at })
  const row = () => triggerView(snapshot(host), store.triggers.get('f1')!)
  expect(row()).toMatchObject({ actions: { requeue: true }, disposition: { delivered: true } })
  // A ticket that ended after its worker had it leaves it to nobody: a person may still send it to Eval.
  store.tickets.save({ ...store.tickets.get('t1')!, ended: { at, by: 'executor', reason: 'done', outcome: 'completed' } })
  expect(row()).toMatchObject({ actions: { requeue: true }, disposition: { delivered: false } })
  // Not once its worker acknowledged it, which took it up; a stop on it decided only the ticket's work.
  store.triggers.saveDisposition('f1', { fanout: ['t1'], acked: ['t1'], at })
  expect(row().actions.requeue).toBe(false)
  store.decisions.save({ id: 'stop', evalId: 'e', threadKey: 'feishu:oc_1:om_root', triggerIds: ['f1'], policyVersion: 'v', at, kind: 'stop',
    reason: [{ code: 'humanStop' }], strength: 'explicit', ticketId: 't1' })
  store.triggers.saveDisposition('f1', { fanout: ['t1'], decisionId: 'stop', at })
  expect(row().actions.requeue).toBe(true)
  // So once the stop ended the ticket, which let go of the message, as the Engine records it.
  store.triggers.saveDisposition('f1', { fanout: [], decisionId: 'stop', at })
  expect(row().actions.requeue).toBe(true)
  // Once sent, it waits for the Eval.
  store.threads.save({ ...store.threads.get('feishu:oc_1:om_root')!, pending: ['f1'] })
  store.triggers.saveDisposition('f1', { fanout: ['t1'], requeued: true, at })
  expect(row()).toMatchObject({ actions: { requeue: false }, disposition: { queued: true } })
})

it('reads each ticket a page\'s follow-ups reached once, and nothing for a message no ticket can decide', () => {
  const host = views()
  live('t1')
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: ['t1'],
    correlated: [], pending: [], meta: {} })
  for (const id of ['f1', 'f2', 'f3']) {
    store.triggers.save(message(id, { source: 'feishu', id: 'ou_alice' }))
    store.triggers.saveDisposition(id, { fanout: ['t1'], at })
  }
  const view = snapshot(host)
  const tickets = vi.spyOn(store.tickets, 'get')
  expect(['f1', 'f2', 'f3'].map(id => triggerView(view, store.triggers.get(id)!).disposition.delivered)).toEqual([true, true, true])
  expect(tickets).toHaveBeenCalledOnce()
  tickets.mockRestore()
  // A message the ticket handed back is decided by no ticket, which needs no read.
  store.triggers.saveDisposition('f3', { fanout: ['t1'], handbacks: [{ ticketId: 't1', reason: 'Another change', at }], at })
  const records = { tickets: { get: vi.fn() }, threads: { get: vi.fn() }, triggers: store.triggers }
  expect(followupOwners(store.triggers.get('f3')!, records)).toEqual([])
  expect([records.tickets.get, records.threads.get].map(read => read.mock.calls.length)).toEqual([0, 0])
})


it('lists Threads by each filter, with paging and latest-activity ordering', () => {
  const host = views()
  live('t1')
  const now = new Date().toISOString()
  const old = '2000-01-01T00:00:00.000Z'
  const thread = (key: string, overrides: Record<string, unknown> = {}) => {
    store.threads.save({
      key, ref: { source: 'feishu', id: key }, createdAt: old, subscriptions: [], correlated: [], pending: [], meta: {}, ...overrides,
    })
  }
  thread('recent', { lastAt: now })
  thread('old')
  thread('subscribed', { subscriptions: ['t1'], lastAt: old })
  thread('evaluating', { evaluating: { id: 'e1', threadKey: 'evaluating', triggerIds: ['f1'], primary: 'f1', strength: 'explicit',
    actor: { source: 'feishu', id: 'ou_alice' }, basis: { via: 'listed' }, policyVersion: 'v', startedAt: old, deadline: old } })
  thread('idle', { subscriptions: ['t1'] })
  thread('owned', { subscriptions: ['t1'], pending: ['f-owner'] })
  store.triggers.save(message('f-owner', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f-owner', { fanout: ['t1'], at })
  expect(threadList(host, 'recent', 0, 10).items.map(item => item.key)).toEqual(['recent'])
  expect(threadList(host, 'all', 0, 1)).toMatchObject({ total: 6, next_offset: 1, items: [{ key: 'recent' }] })
  expect(threadList(host, 'subscribed', 0, 10).items.map(item => item.key)).toEqual(['subscribed', 'idle', 'owned'])
  expect(threadList(host, 'evaluating', 0, 10).items.map(item => item.key)).toEqual(['evaluating'])
})

it('returns no Thread detail for an unknown key and names a missing correlated Thread by its key', () => {
  const host = views()
  expect(threadDetail(host, 'missing', undefined, 10)).toBeUndefined()
  store.threads.save({ key: 'github:known', ref: { source: 'github', id: 'known' }, createdAt: at, subscriptions: ['gone-ticket'], correlated: ['github:gone', 'github:other'],
    pending: [], meta: {} })
  store.triggers.save({ id: 'f1', source: 'github', threads: [{ source: 'github', id: 'known' }], actor: { source: 'github', id: 'alice' },
    human: true, at, text: 'please', channel: githubChannel(42), meta: {} })
  store.threads.save({ key: 'github:other', ref: { source: 'github', id: 'other' }, createdAt: at, subscriptions: [], correlated: [], pending: [], meta: {} })
  const detail = threadDetail(host, 'github:known', undefined, 10)!
  expect(detail.correlated).toEqual([{ key: 'github:gone', title: 'github:gone' }, { key: 'github:other', title: 'github:other' }])
  expect(detail.tickets).toEqual([])
  expect(detail.url).toBe(githubLink({ number: '42' }, {}).url)
})

it('returns no search rows for an empty query and no next decisions at a page boundary', () => {
  const host = views()
  expect(search(host, '   ', { entries: () => new Map<string, string>().entries() })).toEqual({ tickets: [], items: [], threads: [] })
  for (const id of ['d1', 'd2', 'd3']) store.decisions.save({ id, evalId: id, threadKey: 'feishu:oc_1:om_root', triggerIds: [], policyVersion: 'v',
    at, kind: 'ignore', reason: [], strength: 'explicit' })
  expect(decisionList(host, ['ignore'], 0, 1)).toMatchObject({ items: [{ id: 'd3' }], next_offset: 1 })
  expect(decisionList(host, ['ignore'], 2, 1)).toMatchObject({ items: [{ id: 'd1' }], next_offset: null })
})

it('ends a page exactly at the last row and counts a queued Trigger whose record is missing', () => {
  const host = views()
  live('t1')
  expect(page(['a', 'b'], 1, 1)).toEqual({ items: ['b'], total: 2, next_offset: null })
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: ['t1'],
    correlated: [], pending: ['gone', 'f1'], meta: {} })
  store.triggers.save(message('f1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f1', { fanout: ['t1'], at })
  expect(awaitingEval(snapshot(host), store.threads.get('feishu:oc_1:om_root')!)).toEqual(['gone'])
})

it('names a hand-back by the missing ticket id and reports a Thread poll error', () => {
  const host = views()
  store.triggers.save(message('f1', { source: 'feishu', id: 'ou_alice' }))
  store.triggers.saveDisposition('f1', { fanout: [], handbacks: [{ ticketId: 'gone', reason: 'Unrelated', at }], at })
  expect(triggerView(snapshot(host), store.triggers.get('f1')!).disposition.handbacks).toEqual([{ ticketId: 'gone', title: 'gone',
    reason: 'Unrelated', at }])
  store.threads.save({ key: 'feishu:broken', ref: { source: 'feishu', id: 'broken' }, createdAt: at, subscriptions: [], correlated: [],
    pending: [], meta: { pollError: 'token expired' } })
  expect(threadSummary(snapshot(host), store.threads.get('feishu:broken')!).stage).toBe('error')
})

it('shows delivery and failed notes for tickets waiting on a person', () => {
  views()
  live('delivery')
  store.tickets.save({ ...store.tickets.get('delivery')!, delivery: { error: 'worker Session busy', at } })
  expect(ticketSummary(store.tickets.get('delivery')!)).toMatchObject({ note: { text: 'worker Session busy', error: true } })
  live('failed')
  store.tickets.save({ ...store.tickets.get('failed')!, assignments: [{ id: 'a1', state: 'running', createdAt: at, startedAt: at,
    report: { state: 'failed', summary: 'tests failed', at } }] })
  expect(ticketSummary(store.tickets.get('failed')!)).toMatchObject({ note: { text: 'tests failed', error: true } })
})

it('omits a note for a ready report with no summary and names an unknown subscribed ticket', () => {
  const host = views()
  live('review')
  store.tickets.save({ ...store.tickets.get('review')!, assignments: [{ id: 'a1', state: 'running', createdAt: at, startedAt: at,
    report: { state: 'ready', summary: '', at } }] })
  expect(ticketSummary(store.tickets.get('review')!)).not.toHaveProperty('note')
  store.threads.save({ key: 'feishu:unknown-ticket', ref: { source: 'feishu', id: 'unknown-ticket' }, createdAt: at,
    subscriptions: ['gone-ticket'], correlated: [], pending: [], meta: {} })
  expect(threadSummary(snapshot(host), store.threads.get('feishu:unknown-ticket')!).tickets)
    .toEqual([{ id: 'gone-ticket', title: 'gone-ticket', state: 'unknown' }])
})

it('finds search terms cached by their message link and shows a filtered Trigger with no Thread', () => {
  const host = views()
  const links = { entries: () => new Map<string, string>([['feishu:oc_1:om_9', 'https://example.com/link']]).entries() }
  expect(search(host, 'https://example.com/link', links).items).toEqual([])
  store.triggers.save({ ...message('f1', { source: 'feishu', id: 'ou_bob' }), text: 'needle', human: false,
    threads: [{ source: 'feishu', id: 'gone' }] })
  store.triggers.saveDisposition('f1', { filtered: { code: 'chat_not_allowed' }, fanout: [], at })
  store.threads.save({ key: 'feishu:oc_1:om_root', ref: { source: 'feishu', id: 'oc_1:om_root' }, createdAt: at, subscriptions: [], correlated: [], pending: [], meta: {} })
  store.triggers.save({ ...message('f2', { source: 'feishu', id: 'ou_bob' }), text: 'needle', at: '2000-01-01T00:00:00.000Z' })
  store.triggers.saveDisposition('f2', { fanout: [], at })
  store.triggers.save({ ...message('f3', { source: 'feishu', id: 'ou_bob' }, true, at), text: 'needle' })
  store.triggers.saveDisposition('f3', { fanout: [], at })
  expect(search(host, 'other', links).items).toEqual([])
  const rows = search(host, 'needle', { entries: () => new Map<string, string>().entries() }).items
  expect(rows.map(item => item.id)).toEqual(expect.arrayContaining(['f1', 'f2', 'f3']))
  const third = rows.find(item => item.id === 'f3')!
  expect(third).not.toHaveProperty('filtered')
  expect(third).toMatchObject({ threadTitle: 'needle' })
  store.triggers.save({ ...message('f4', { source: 'feishu', id: 'ou_bob' }, true, at), text: 'needle', threads: [{ source: 'feishu', id: 'gone' }] })
  store.triggers.saveDisposition('f4', { fanout: [], at })
  const fallback = search(host, 'needle', { entries: () => new Map<string, string>().entries() }).items.find(item => item.id === 'f4')!
  expect(fallback).toMatchObject({ filtered: '', threadTitle: undefined })
  expect(rows.find(item => item.id === 'f1')).toMatchObject({ filtered: { code: 'chat_not_allowed' }, threadTitle: undefined })
})

it('projects decisions from current and legacy audit records, including missing related records', () => {
  const host = views()
  const view = snapshot(host)
  const base: Omit<Decision, 'id' | 'audit' | 'ticketId'> = { evalId: 'e1', threadKey: 'github:gone', triggerIds: ['gone'],
    policyVersion: 'v', at, kind: 'ignore', reason: [], strength: 'inferred' }
  const decision = (id: string, audit: Record<string, unknown> = {}): Decision => ({ id, ...base, audit, ...id === 'ticketed' ? { ticketId: 'gone-ticket' } : {} })
  expect(decisionView(view, decision('would', { would_accept: true }))).toMatchObject({ outcome: 'wouldAccept' })
  expect(decisionView(view, decision('legacy', { matches: [{ rule_id: 'copy', reason: 'matched' }] }))).toMatchObject({
    rules: [{ id: 'copy', name: 'Copy fixes', matched: true, reason: 'matched' }], preview: '',
  })
  expect(decisionView(view, decision('ticketed', { direction: { implementation_requested: true, target_basis: 'message',
    requires_design_decision: false, evidence: 'the policy' } }))).toMatchObject({ direction: { implementation_requested: true },
    ticketTitle: undefined })
  expect(decisionView(view, decision('ticketed'))).not.toHaveProperty('direction')
})

it('prefers the Trigger URL for a Channel link that names one', () => {
  const host = views()
  store.triggers.save(message(feishuTriggerId('om_f1'), { source: 'feishu', id: 'ou_alice' }))
  const trigger = store.triggers.get(feishuTriggerId('om_f1'))!
  store.triggers.save({ ...trigger, url: 'https://example.com/message' })
  expect(channelLink(host, feishuChannel('oc_1', 'om_f1'))).toMatchObject({ source: 'feishu', url: 'https://example.com/message' })
})

it('formats every Mandate admission fact and legacy authority', () => {
  const rule = (id: string) => ({ id, name: id === 'copy' ? 'Copy fixes' : id })
  expect(basisFacts({ via: 'followup', authority: 'message 7' }, rule)).toEqual({ facts: [{ name: 'legacyAuthority', value: 'message 7' }], legacy: true })
  expect(basisFacts({ via: 'followup' }, rule)).toEqual({ facts: [{ name: 'followup' }], legacy: false })
  expect(basisFacts({ via: 'admission', cell: 'listedUnaddressed', addressed: 'mention', observation: { mode: 'observe' } }, rule).facts)
    .toEqual([{ name: 'addressed', value: 'mention' }, { name: 'admission', value: 'listedUnaddressed' }, { name: 'observation' },
      { name: 'mode', value: 'observe' }])
  expect(basisFacts({ via: 'listed', addressed: 'direct', observation: { mode: 'auto' }, authority: 'message 8', rules: ['copy'] }, rule))
    .toEqual({ facts: [{ name: 'addressed', value: 'direct' }, { name: 'observation' }, { name: 'mode', value: 'auto' },
      { name: 'rules', value: [{ id: 'copy', name: 'Copy fixes' }] }, { name: 'legacyAuthority', value: 'message 8' }], legacy: true })
})

it('lists ended tickets, queues executor load, and returns no detail for an unknown ticket', () => {
  const host = views()
  for (const id of ['e1', 'e2']) {
    live(id)
    store.tickets.save({ ...store.tickets.get(id)!, ended: { at, by: 'executor', outcome: 'completed' } })
  }
  expect(ticketList(host, 'ended', 0, 1)).toMatchObject({ total: 2, next_offset: 1, items: [{ id: 'e2' }] })
  live('queued')
  store.tickets.save({ ...store.tickets.get('queued')!, assignments: [{ id: 'a1', state: 'pending', createdAt: at }] })
  expect(executorLoad(store, 3)).toEqual({ running: 3, queued: 1 })
  expect(ticketDetail(host, 'missing')).toBeUndefined()
})

it('formats a ticket Mandate rule list and omits a Session ref for an Assignment that never started', () => {
  const host = views()
  live('rules')
  store.tickets.save({ ...store.tickets.get('rules')!, mandate: { ...store.tickets.get('rules')!.mandate,
    basis: { via: 'listed', rules: ['copy'] } }, assignments: [{ id: 'a1', state: 'pending', createdAt: at }] })
  const detail = ticketDetail(host, 'rules')!
  expect(detail.basis).toMatchObject({ facts: [{ name: 'rules', value: [{ id: 'copy', name: 'Copy fixes' }] }] })
  expect(detail.assignments[0]).not.toHaveProperty('ref')
})

it('adds a reply link and renders text Inputs for a ticket waiting on a person', () => {
  const host = views()
  live('t1')
  store.tickets.save({ ...store.tickets.get('t1')!, channels: [feishuChannel('oc_1', 'om_f1')], attention: { id: 'a1', question: 'Which page?' },
    inputs: [{ id: 'i1', kind: 'text', text: 'look at page 2', at }, { id: 'i2', kind: 'note', text: 'the build failed', at }] })
  const rows = ticketList(host, 'person', 0, 10).items
  expect(rows[0]).toMatchObject({ id: 't1', reply: expect.objectContaining({ source: 'feishu' }) as unknown })
  const detail = ticketDetail(host, 't1')!
  expect(detail.inputs.map(input => input.text)).toEqual(['the build failed', 'look at page 2'])
  expect(detail.decision).toBeUndefined()
})

it('projects settings blocks, Source forms, policy metadata, and the execution environment', () => {
  const policySchema = z.object({
    room: z.string().default(''),
    allowed: z.array(z.object({ id: z.string(), name: z.string().optional() }).strict()).default([]),
  }).strict()
  const plain: SourceKind<{ poll: number }> = {
    kind: 'github', title: { zh: 'GitHub', en: 'GitHub' }, description: { zh: 'GitHub 描述', en: 'GitHub description' },
    health: [], schema: z.object({ poll: z.number().default(1) }).strict(),
    fields: [{ key: 'poll', type: 'number', min: 1, max: 60, label: { zh: '轮询', en: 'Poll' } }],
    facts: () => ({ repository: 'acme/widgets' }),
    policy: { schema: policySchema, fields: [], explicit: { zh: '', en: '' } },
  }
  const rich: SourceKind = {
    kind: 'probe', title: { zh: '探针', en: 'Probe' }, description: { zh: '', en: '' }, health: [], schema: z.object({}).strict(),
    fields: [], facts: () => ({}),
    policy: {
      schema: policySchema, fields: [], explicit: { zh: '', en: '' },
      rows: () => [{ id: 'room', label: { zh: '房间', en: 'Room' }, note: { zh: '', en: '' }, cells: { listedUnaddressed: false, unlistedAddressed: false, unlistedUnaddressed: false } }],
      sync: { label: { zh: '同步', en: 'Sync' }, ready: () => true, run: async () => policySchema.parse({ room: 'room' }) },
    },
  }
  store.saveSettings(SettingsSchema.parse({
    sources: {
      github: { enabled: true, config: {} }, probe: { enabled: true, config: {} }, retired: { enabled: true, config: { keep: true } },
    },
    policy: { sources: { github: { room: 'chat' }, probe: { room: 'room' } } },
  }))
  const sources = new SourceRegistry(() => store.settings())
  sources.register(plain); sources.register(rich)
  const setup = { deployment: testDeployment(), problem: { place: 'github' as const, code: 'repositoryUnset' as const, values: {}, detail: 'repository unset' } }
  const view = settingsView(store, sources, { 'sync:probe': { state: 'ready', detail: 'ok' } }, setup, '/state')
  expect(view.policyVersion).toBeTruthy()
  expect(view.blocks).toEqual({
    policy: { value: store.settings().policy, revision: 0 },
    eval: { value: store.settings().eval, revision: 0 },
    executor: { value: store.settings().executor, revision: 0 },
  })
  expect(view.sources).toEqual([
    expect.objectContaining({ id: 'github', kind: 'github', installed: true, restarts: ['repository', 'cli'], fields: plain.fields, facts: { repository: 'acme/widgets' }, problem: setup.problem, policy: { fields: [], explicit: { zh: '', en: '' } } }),
    expect.objectContaining({ id: 'probe', kind: 'probe', installed: true, restarts: [], fields: [], facts: {}, policy: { fields: [], explicit: { zh: '', en: '' }, rows: [expect.anything()], sync: { label: { zh: '同步', en: 'Sync' }, ready: true, health: { state: 'ready', detail: 'ok' } } } }),
    expect.objectContaining({ id: 'retired', kind: 'retired', installed: false, description: { zh: '', en: '' }, restarts: [], fields: [], facts: {} }),
  ])
  expect(view.environment.root).toBe('/state/work')
  expect(Array.isArray(view.environment.variables)).toBe(true)
})
