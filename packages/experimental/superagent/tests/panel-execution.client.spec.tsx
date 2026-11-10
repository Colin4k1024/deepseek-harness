// @vitest-environment jsdom
// The Execution status, a Ticket, and the Execution settings.
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { expect, it, onTestFinished, vi } from 'vitest'
import { en, zh } from '../src/client/locales.ts'
import { PanelProvider } from '../src/client/provider.tsx'
import { ExecutionSettings } from '../src/client/execution.tsx'
import { SettingsProvider } from '../src/client/settings-kit.tsx'
import { TicketPage } from '../src/client/ticket.tsx'
import { translator, usePanel, mount, remount, settle, all, find, tab, click, type, text, field, at } from './fixtures/panel.client.tsx'
import { MEMBER, actor, problemOf, server, ticketSummary, threadSummary } from './fixtures/panel-server.client.ts'

usePanel()
const requestUrl = (input: RequestInfo | URL): string => input instanceof URL ? input.href : input instanceof Request ? input.url : input
it('lists Tickets under Execution by who they wait on, with the Executor state and its pause as an action', async () => {
  server.tickets.open = [ticketSummary(), ticketSummary({ id: 't2', title: '执行中的工单', threads: [{ key: 'github:7',
    source: 'github', title: '#7 x', facts: { number: { value: '#7' }, autoMerge: { label: { zh: '自动合并', en: 'Auto-merge' },
      value: { zh: '已开启', en: 'on' } } } }] })]
  await mount()
  await click(zh['tab.execution'])
  expect(document.querySelector('.dt-status-title')?.textContent).toBe(zh.statusTitle)
  expect(server.requests.some(request => request.path === 'tickets?state=open&limit=20')).toBe(true)
  expect(text()).toContain(zh.executorRunning)
  expect(all('.dt-chip').map(chip => chip.textContent)).toContain('GitHub#7自动合并已开启')
  await click(zh.idleLabel)
  expect(server.requests.some(request => request.path === 'tickets?state=idle&limit=20')).toBe(true)
  expect(text()).toContain('执行器结束了当前回合')
  await click(zh['ticket.ended'])
  expect(text()).toContain('旧工单')
  expect(text()).toContain(zh['outcome.stopped'])
  expect(all('.dt-status-line .dt-lv').map(item => item.textContent)).toEqual(['执行中 2 / 3', '排队 0'])
  // The pause is its own action: an Execution settings draft started before it still saves.
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  expect(tab(zh['tab.settings']).getAttribute('aria-selected')).toBe('true')
  expect(document.querySelector('.dt-settings-link[aria-current="page"]')?.textContent).toBe(zh['promptPart.runtime/work'])
  await type(field<HTMLInputElement>(zh.maxWorkers), '4')
  await click(tab(zh['tab.execution']))
  await click(zh.pauseProcessing)
  await click(zh.confirm)
  expect(server.requests.find(request => request.path === 'executor/pause')).toMatchObject({ method: 'POST', body: { paused: true } })
  expect(text()).toContain(zh.processingPaused)
  expect(find(zh.resumeProcessing).textContent).toBe(zh.resumeProcessing)
  await click(tab(zh['tab.settings']))
  await click(zh.save)
  expect(server.settings).toMatchObject({ paused: true, executor: { maxWorkers: 4 } })
})

it('stops an open Ticket from its page after confirmation', async () => {
  await mount()
  await click('修复回归')
  await click(zh.stop)
  expect(server.requests.some(request => request.path.endsWith('/stop'))).toBe(false)
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain('停止“修复回归”')
  await click(zh.confirm)
  expect(server.requests.some(request => request.path === 'tickets/t1/stop' && request.method === 'POST')).toBe(true)
})

it('shows a Ticket with its stage, its Mandate details with the Trigger and Decision behind it, its Threads, and the rest behind disclosures', async () => {
  await mount()
  await click(all('.dt-open').find(button => button.textContent === '修复回归')!)
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('修复回归')
  // Progress is in the Channels and the Session, so the page has no report section.
  expect(all('.dt-section-title').map(title => title.querySelector('.dt-section-name')?.textContent)).toEqual(['授权详情', '未送达的输入', '订阅的线程'])
  expect(document.querySelector('.dt-status-line .dt-stage')?.textContent).toBe(zh.executingLabel)
  expect(document.querySelector('.dt-status-line .dt-stage .dt-spinner')).not.toBeNull()
  // A Thread this Ticket follows reads as subscribed on its page, though no other Ticket follows it.
  expect(all('.dt-list .dt-ticket .dt-stage').map(stage => stage.textContent)).toContain(zh.threadSubscribed)
  expect(all('.dt-list .dt-ticket .dt-stage').map(stage => stage.textContent)).not.toContain(zh.threadUnsubscribed)
  expect(all('.dt-status-line .dt-lv').map(item => item.textContent)).toEqual(['授权 明确', '发起 飞书 · Alice', expect.stringMatching(/^更新 /)])
  for (const expected of ['修复登录页回归', '点名方式@ 智能体', '名单里的人明确请求，由主 Session 接单', '未送达：等待派工运行',
    'Alice：顺便改标题']) expect(text()).toContain(expected)
  expect(all('.dt-chip').map(chip => chip.textContent)).toContain('飞书团队群')
  await click(zh['term.channel'])
  expect(all('.dt-disclosure-body a').map(link => [link.textContent, link.getAttribute('href')])).toEqual([['飞书消息',
    'https://applink.feishu.cn/client/chat/open?openChatId=oc_1'], ['GitHub #42', 'https://github.com/acme/widgets/issues/42']])
  await click(zh['term.environment'])
  expect(all('.dt-disclosure-body .dt-table tr').map(row => row.textContent)).toEqual(['DSH_SUPERAGENT_DIR/w/t1',
    'DSH_SUPERAGENT_GITHUB_NUMBER—'])
  await click(document.querySelector<HTMLElement>('.dt-trigger-link')!)
  expect(tab(zh['tab.trigger']).getAttribute('aria-selected')).toBe('true')
})

it('reports a missing Ticket instead of rendering an empty page', async () => {
  at({ page: 'execution', id: 'missing' })
  await mount()
  expect(document.querySelector('.dt-hint')?.textContent).toBe(zh.ticketNotFound)
})

it('links a question to its Channel, folds consecutive automated messages among Inputs, and shows field explanations in a hover tooltip',
  async () => {
    const ci = (id: string, at: string, failed: number) => ({ id, kind: 'trigger' as const, triggerId: `c${id}`, at, delivered: true,
      trigger: { id: `c${id}`, actor: { source: 'github', id: '' }, human: false,
        preview: `CI of PR #42 at abc1234: 10 checks, ${failed} failed, 0 running.`, thread: 'github:42' } })
    const t1 = server.ticketDetails.t1!
    server.ticketDetails.t1 = { ...t1, stage: 'person', action: 'answer', attention: { id: 'q1', question: '需要确认按钮颜色' }, inputs: [ci('i3',
      '2026-09-22T00:06:00Z', 1), ci('i2', '2026-09-22T00:05:00Z', 0)] }
    await mount()
    await click(all('.dt-open').find(button => button.textContent === '修复回归')!)
    expect(all('.dt-banner a').map(link => [link.textContent, link.getAttribute('href')])).toEqual([['去飞书回答',
      'https://applink.feishu.cn/client/chat/open?openChatId=oc_1']])
    await click('已送达的输入 · 2')
    expect(all('.dt-disclosure-body .dt-row-title').map(title => title.textContent)).toEqual(['自动通知：CI of PR #42 at abc1234: 10 checks, 1 failed, 0 running.'])
    await click('同一发送者之前还有 1 条自动消息')
    expect(text()).toContain('CI of PR #42 at abc1234: 10 checks, 0 failed, 0 running.')
    await click(zh['tab.eval'])
    await click(find(zh['tab.settings'], '.dt-status-head button'))
    expect(text()).not.toContain('到期后记为未完成')
    const tip = field(zh.evalTimeout).closest('.dt-field')!.querySelector<HTMLElement>('.dt-info')!
    await act(async () => { tip.dispatchEvent(new MouseEvent('mouseover', { bubbles: true })) })
    expect(document.querySelector('[role="tooltip"]')?.textContent).toContain('到期后记为未完成')
    expect(tip.closest('.dt-field')!.textContent).not.toContain('到期后记为未完成')
    await act(async () => { tip.dispatchEvent(new MouseEvent('mouseout', { bubbles: true })) })
    expect(document.querySelector('[role="tooltip"]')).toBeNull()
  })

it('offers a Ticket awaiting review its review place and Session in the page head', async () => {
  const t1 = server.ticketDetails.t1!
  server.ticketDetails.t1 = { ...t1, stage: 'person', action: 'review', threads: [threadSummary(),
    { ...threadSummary({ key: 'github:42', source: 'github', title: '#42 Fix login' }), url: 'https://github.com/acme/widgets/pull/42' }] }
  at({ page: 'execution', id: 't1' })
  await mount()
  expect(document.querySelector('.dt-status-line .dt-stage')?.textContent).toBe(zh['stage.review'])
  expect(all('.dt-page-head a').map(link => [link.textContent, link.getAttribute('href')])).toEqual([['打开 GitHub 原文',
    'https://github.com/acme/widgets/pull/42']])
})

it('keeps what a person types in the Execution settings, converting it once on save', async () => {
  await mount()
  await click(zh['tab.execution'])
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  const blocked = () => field<HTMLInputElement>(zh.blockedCommands), root = () => field<HTMLInputElement>(zh.executionDirectory),
    workers = () => field<HTMLInputElement>(zh.maxWorkers)
  // A separator typed before the next command, a space inside a path and a cleared number stay as typed.
  await type(blocked(), 'gh-personal ')
  expect(blocked().value).toBe('gh-personal ')
  await type(blocked(), 'gh-personal, gh-other')
  await type(root(), '/srv/my work ')
  expect(root().value).toBe('/srv/my work ')
  await type(workers(), '')
  expect(workers().value).toBe('')
  await type(workers(), '2')
  await click(zh.save)
  expect(server.requests.find(request => request.path === 'settings/executor')?.body).toMatchObject({ value: { blockedCommands: ['gh-personal', 'gh-other'], directory: '/srv/my work ', maxWorkers: 2 } })
  expect(server.settings.executor).toMatchObject({ blockedCommands: ['gh-personal', 'gh-other'], directory: '/srv/my work', maxWorkers: 2 })
})

it('says why an Input was not delivered on its Ticket\'s page, in either language', async () => {
  const t1 = server.ticketDetails.t1!
  server.ticketDetails.t1 = { ...t1, stage: 'person', action: 'delivery', deliveryError: 'feishu: message recalled' }
  at({ page: 'execution', id: 't1' })
  await mount()
  expect(document.querySelector('.dt-status-line .dt-stage')?.textContent).toBe(zh['stage.delivery'])
  expect(document.querySelector('.dt-banner.dt-danger')?.textContent).toBe(`${zh.deliveryFailedPrefix}feishu: message recalled`)
  expect(text()).toContain(zh.inputRetryHint)
  expect(text()).not.toContain(zh.inputWaitingHint)
  expect(document.querySelector('.dt-status-line')?.textContent).toMatch(/9月\d+日/)
  await remount(en)
  expect(document.querySelector('.dt-status-line')?.textContent).toMatch(/Sep \d+/)
  expect(document.querySelector('.dt-status-line .dt-stage')?.textContent).toBe(en['stage.delivery'])
  expect(document.querySelector('.dt-banner.dt-danger')?.textContent).toBe(`${en.deliveryFailedPrefix}feishu: message recalled`)
  expect(text()).toContain(en.inputRetryHint)
})

it('tells a Ticket or Thread that does not exist from a dsh-superagent that is not answering', async () => {
  at({ page: 'execution', id: 't9' })
  await mount()
  expect(text()).toContain(zh.ticketNotFound)
  // While dsh-superagent restarts, DSH's web server answers every route with an empty 404.
  at({ page: 'execution', id: 't1' })
  server.down = true
  await remount()
  expect(text()).not.toContain(zh.ticketNotFound)
  expect(text()).toContain(zh.hostNotRunning)
  at({ page: 'trigger', id: 'feishu:oc_1:om_root' })
  await remount()
  expect(text()).not.toContain(zh.threadNotFound)
  expect(text()).toContain(zh.hostNotRunning)
})

it('says when the Execution list has no Tickets', async () => {
  server.tickets.open = []
  await mount()
  await click(zh['tab.execution'])
  expect(text()).toContain(zh.none)
})

it('keeps the Execution status while the Ticket list itself fails to load', async () => {
  await mount()
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => requestUrl(input).includes('/tickets?')
    ? new Response(null, { status: 500 })
    : original(input, init))
  await click(zh['tab.execution'])
  expect(text()).toContain(zh.serverError.replace('{status}', '500'))
  expect(text()).toContain(zh.executorRunning)
})

it('gives a Ticket a bot\'s message started its basis, marks a Thread it follows for context, and an Input it handed back', async () => {
  const t1 = server.ticketDetails.t1!
  server.ticketDetails.t1 = { ...t1, mandate: { ...t1.mandate, strength: 'inferred', basis: { via: 'bot', sender: '评审机器人',
    observation: { mode: 'auto' } } },
  basis: { facts: [{ name: 'sender', value: '评审机器人' }, { name: 'bot' }, { name: 'mode', value: 'auto' }], legacy: false },
  threads: [threadSummary(), { ...threadSummary({ key: 'github:7', title: '#7 相关 Issue' }), context: true }],
  inputs: [{ id: 'i1', kind: 'trigger', triggerId: 'e2', at: '2026-09-22T00:03:00Z', delivered: true, trigger: { id: 'e2',
    actor: { source: 'feishu', id: MEMBER, name: { zh: 'Alice', en: 'Alice' } }, human: true, preview: '顺便加一个页面',
    thread: 'feishu:oc_1:om_root', handback: { reason: 'Asks for a new page', at: '2026-09-22T00:05:00Z' } } }] }
  at({ page: 'execution', id: 't1' })
  await mount()
  const facts = () => all('.dt-kv > div').map(row => [row.querySelector('dt')?.textContent, row.querySelector('dd')?.textContent])
  expect(facts()).toEqual(expect.arrayContaining([[zh['basis.sender'], '评审机器人'], [zh['basis.bot'], zh.yes], [zh['basis.mode'], zh['mode.auto']]]))
  expect(all('.dt-list .dt-ticket').map(row => row.querySelector('.dt-tag')?.textContent)).toEqual([undefined, zh.contextOnly])
  await click(zh.deliveredInputs.replace('{count}', '1'))
  expect(text()).toContain('已交回评估：Asks for a new page')
  await remount(en)
  expect(facts()).toEqual(expect.arrayContaining([[en['basis.bot'], en.yes]]))
  expect(all('.dt-list .dt-ticket').map(row => row.querySelector('.dt-tag')?.textContent)).toEqual([undefined, en.contextOnly])
  await click(en.deliveredInputs.replace('{count}', '1'))
  expect(text()).toContain('Handed back for an Eval: Asks for a new page')
})

it('words an ended Ticket by its outcome, on its row and its page, with a person\'s reason after it', async () => {
  const fields = { id: 't3', title: '已合并的工单', state: 'ended' as const, actions: { retry: false, stop: false },
    ended: { at: '2026-09-21T00:00:00Z', by: 'executor' as const, outcome: 'merged' as const } }
  server.tickets.ended = [ticketSummary(fields, { stage: 'ended' }), ...server.tickets.ended]
  server.ticketDetails.t3 = { ...server.ticketDetails.t1!, ...fields, stage: 'ended' }
  await mount()
  await click(zh['tab.execution'])
  await click(zh['ticket.ended'])
  expect(all('.dt-ticket-stage').map(stage => stage.textContent)).toEqual([zh['outcome.merged'], zh['outcome.stopped']])
  await click('已合并的工单')
  expect(document.querySelector('.dt-status-line .dt-stage')?.textContent).toBe(zh['outcome.merged'])
  expect(text()).toContain(zh['outcome.merged'])
  expect(text()).not.toContain('执行器结束')
  server.ticketDetails.t3 = { ...server.ticketDetails.t3, ended: { at: '2026-09-21T00:00:00Z', by: 'human', outcome: 'stopped',
    reason: '不需要了' } }
  await remount(en)
  expect(all('.dt-hint').map(hint => hint.textContent).find(line => line?.includes(en['outcome.stopped']))).toMatch(/Stopped by a person: 不需要了$/)
})

it('sends one stop for a double click on Confirm', async () => {
  at({ page: 'execution', id: 't1' })
  await mount()
  await click(zh.stop)
  const confirm = find(zh.confirm)
  await act(async () => { confirm.click(); confirm.click() })
  expect(server.requests.filter(request => request.path === 'tickets/t1/stop')).toHaveLength(1)
})


it('shows a resumed Ticket\'s assignments, every Mandate fact, and its paused Inputs', async () => {
  const routes: object[] = []
  const base = server.ticketDetails.t1!
  server.ticketDetails.t5 = {
    ...base,
    id: 't5', title: '丰富工单', actions: { retry: true, stop: false }, sessionId: '', directory: '',
    stage: 'person', action: 'failed', summary: 'LlmError: no adapter registered', paused: true,
    threads: [{ ...threadSummary(), url: 'https://example.test/thread' }],
    assignments: [
      { id: 'a4', state: 'pending', createdAt: '2026-09-22T00:00:00Z' },
      { id: 'a3', state: 'running', createdAt: '2026-09-22T00:00:00Z', startedAt: '2026-09-22T00:01:00Z',
        turnEndedAt: '2026-09-22T00:04:00Z', report: { state: 'failed', summary: 'failed', at: '2026-09-22T00:04:00Z' } },
      { id: 'a2', state: 'ended', createdAt: '2026-09-22T00:00:00Z', startedAt: '2026-09-22T00:01:00Z',
        endedAt: '2026-09-22T00:03:00Z', endedReason: 'Human asked to stop', ref: 'worker-other',
        report: { state: 'blocked', summary: 'blocked', at: '2026-09-22T00:02:00Z' } },
      { id: 'a1', state: 'ended', createdAt: '2026-09-22T00:00:00Z', endedAt: '2026-09-22T00:02:00Z',
        endedReason: 'Nothing to run', report: { state: 'ready', summary: 'ready', at: '2026-09-22T00:01:00Z' } },
    ],
    inputs: [
      { id: 'i3', kind: 'text', text: 'A person added context', at: '2026-09-22T00:05:00Z', delivered: true },
      { id: 'i2', kind: 'note', text: '', at: '2026-09-22T00:04:00Z', delivered: true },
      { id: 'i1', kind: 'trigger', triggerId: 'e2', at: '2026-09-22T00:03:00Z', delivered: false,
        trigger: { id: 'e2', actor: actor('feishu', MEMBER), human: true, preview: 'resume it', thread: 'feishu:oc_1:om_root' } },
    ],
    basis: {
      facts: [
        { name: 'sender', value: 'Alice' }, { name: 'bot' }, { name: 'mode', value: 'auto' },
        { name: 'addressed', value: 'mention' }, { name: 'admission', value: 'unlistedAddressed' },
        { name: 'rules', value: [{ id: 'r1', name: '文案修改' }] }, { name: 'followup' },
      ],
      legacy: false,
    },
    decision: {
      ...base.decision!,
      preview: '',
      rules: [{ id: 'r1', name: '文案修改', matched: true, reason: 'matches wording' },
        { id: 'r2', name: '交付要求', matched: false, reason: 'does not apply' }],
      direction: { implementation_requested: true, target_basis: 'human', requires_design_decision: true, evidence: 'the message asks for it' },
    },
  }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  const Harness = ({ id, name }: { id: string; name: string }) => (
    <PanelProvider t={translator(zh)} navigate={(route) => { routes.push(route) }}>
      <TicketPage key={name} id={id} />
    </PanelProvider>
  )
  await act(async () => { root.render(<Harness id="t5" name="current" />) }); await settle()
  for (const expected of ['第 4 次派工', '报告：失败', '报告：受阻', zh.inputPausedHint, '文案修改 · 匹配：matches wording',
    '交付要求 · 不匹配：does not apply', '实施意图：已明确；修改依据：human；仍需产品／视觉设计决策', zh.failedPrefix]) expect(text()).toContain(expected)
  await click(document.querySelector('.dt-trigger-link') as HTMLElement)
  expect(routes.at(-1)).toEqual({ page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: 'e1' } })
  await click(document.querySelector('.dt-textlink.dt-mono') as HTMLElement)
  expect(routes.at(-1)).toEqual({ page: 'settings', query: { group: 'eval', version: 'abcdef1234567890' } })
  await click(zh.retry); await click(zh.confirm)
  expect(server.requests.some(request => request.path === 'tickets/t5/retry' && request.method === 'POST')).toBe(true)

  const current = server.ticketDetails.t5
  if (current === undefined) throw new Error('expected the current ticket fixture')
  server.ticketDetails.t5 = {
    ...current,
    mandate: { ...current.mandate, scope: 'legacy scope' },
    basis: { facts: [{ name: 'legacyAuthority', value: 'old authority' }], legacy: true },
    decision: { ...current.decision!, direction: { implementation_requested: false, target_basis: 'rule',
      requires_design_decision: false, evidence: 'the rule says so' } },
  }
  await act(async () => { root.render(<Harness id="t5" name="legacy" />) }); await settle()
  expect(document.querySelector('.dt-textlink.dt-mono')).toBeNull()
  expect(text()).toContain(zh.legacyScopeShort)
  expect(text()).toContain('实施意图：未明确'); expect(text()).toContain('无需新增设计决策')
})

it('keeps the open filter explicit, shows a person Ticket and a setup-blocked Executor, and pages Tickets', async () => {
  server.setup = { ...server.setup, problem: problemOf('agentCliUnset') }
  server.tickets.open = [ticketSummary({}, { stage: 'person', action: 'answer' })]
  server.nextOffset = 20
  at({ page: 'execution' })
  await mount()
  await click(zh['ticketFilter.open'])
  expect(server.requests.filter(request => request.path === 'tickets?state=open&limit=20')).toHaveLength(1)
  expect(text()).toContain(zh.executorWaitingSetup)
  await click(zh.loadMore)
  expect(server.requests.some(request => request.path === 'tickets?state=open&limit=40')).toBe(true)
})

it('opens an undelivered Trigger Input from its Ticket', async () => {
  at({ page: 'execution', id: 't1' })
  await mount()
  await click(find('Alice：顺便改标题', '.dt-open'))
  expect(tab(zh['tab.trigger']).getAttribute('aria-selected')).toBe('true')
  expect(server.requests.some(request => request.path.includes('threads/feishu:oc_1:om_root') && request.path.includes('around=e2'))).toBe(true)
})

it('reports a Ticket list read failure after the panel state has loaded', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => requestUrl(input).includes('/tickets?')
    ? new Response(JSON.stringify({ error: 'ticket store unavailable' }), { status: 500, headers: { 'content-type': 'application/json' } })
    : original(input, init))
  at({ page: 'execution' })
  await mount()
  expect(text()).toContain('ticket store unavailable')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('names an environment variable owner that is no longer listed as a Source', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    if (!requestUrl(input).endsWith('/settings')) return original(input, init)
    const state = await original(input, init)
    const settings = await state.json() as Record<string, unknown>
    return Response.json({ ...settings, environment: { root: '/srv/work', variables: [{ owner: 'removed', enabled: true,
      variables: [{ key: 'DSH_REMOVED', description: { zh: '已移除来源的变量', en: 'Variable from a removed source' } }] }] } })
  })
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  await act(async () => {
    root.render(<PanelProvider t={translator(zh)} navigate={() => {}}>
      <SettingsProvider><ExecutionSettings /></SettingsProvider>
    </PanelProvider>)
  })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  const variables = [...container.querySelectorAll<HTMLElement>('[data-disclosure-row]')].find(row => row.textContent?.includes(zh.environmentVariables))!
  await act(async () => { variables.click() })
  expect(text()).toContain('removed')
})

it('shows a Ticket without a Decision, an empty scope, mixed Inputs, and a plain blocked summary', async () => {
  const base = server.ticketDetails.t1!
  server.ticketDetails.t6 = {
    ...base, id: 't6', title: '等待仓库维护者', actions: { retry: true, stop: false }, summary: 'waiting for repository owner',
    stage: 'person', action: 'review', decision: undefined, threads: [], environment: [], channels: [],
    mandate: { ...base.mandate, scope: '' }, basis: { facts: [], legacy: false },
    assignments: [
      { id: 'a2', state: 'ended', createdAt: '2026-09-22T00:00:00Z', startedAt: '2026-09-22T00:01:00Z',
        endedAt: '2026-09-22T00:03:00Z', report: { state: 'ready', summary: 'ready', at: '2026-09-22T00:02:00Z' } },
      { id: 'a1', state: 'running', createdAt: '2026-09-22T00:00:00Z', startedAt: '2026-09-22T00:01:00Z',
        report: { state: 'progress', summary: 'working', at: '2026-09-22T00:02:00Z' } },
    ],
    inputs: [
      { id: 'i4', kind: 'text', text: '', at: '2026-09-22T00:06:00Z', delivered: true },
      { id: 'i3', kind: 'policy-changed', at: '2026-09-22T00:05:00Z', delivered: true,
        from: 'aaaaaaaaaaaaaaaa', to: 'bbbbbbbbbbbbbbbb' },
      { id: 'i2', kind: 'trigger', triggerId: 'missing', at: '2026-09-22T00:04:00Z', delivered: false },
      { id: 'i1', kind: 'note', text: 'A person added context', at: '2026-09-22T00:03:00Z', delivered: false },
      { id: 'i0', kind: 'trigger', triggerId: 'empty', at: '2026-09-22T00:02:00Z', delivered: false,
        trigger: { id: 'empty', actor: actor('feishu', MEMBER), human: true, preview: '', thread: 'feishu:oc_1:om_root' } },
    ],
  }
  at({ page: 'execution', id: 't6' })
  await mount()
  expect(text()).toContain(`${zh.blockedPrefix}waiting for repository owner`)
  expect(text()).not.toContain(zh.failedPrefix)
  expect(all('.dt-kv > div').find(row => row.querySelector('dt')?.textContent === zh['term.scope'])?.querySelector('dd')?.textContent).toBe(zh.empty)
  expect(text()).toContain(zh['input.note'])
  expect(text()).toContain(zh['input.trigger'])
  await click(zh.deliveredInputs.replace('{count}', '2'))
  expect(text()).toContain(zh['input.policy-changed'])
  expect(all('.dt-list .dt-row .dt-tag').map(tag => tag.textContent)).toContain(zh['assignment.running'])
})

it('opens the Thread of a Decision that names no Trigger', async () => {
  const base = server.ticketDetails.t1!
  const routes: object[] = []
  server.ticketDetails.t7 = { ...base, id: 't7', decision: { ...base.decision!, triggerIds: [], preview: '' } }
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  await act(async () => {
    root.render(<PanelProvider t={translator(zh)} navigate={(route) => { routes.push(route) }}><TicketPage id="t7" /></PanelProvider>)
  })
  await settle()
  await click(document.querySelector<HTMLElement>('.dt-trigger-link')!)
  expect(routes.at(-1)).toEqual({ page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: '' } })
})

it('keeps showing a Ticket after a later poll fails', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
  onTestFinished(() => { vi.useRealTimers() })
  at({ page: 'execution', id: 't1' })
  await mount()
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) =>
    requestUrl(input).includes('/tickets/t1')
      ? new Response(JSON.stringify({ error: 'ticket read failed' }), { status: 500, headers: { 'content-type': 'application/json' } })
      : original(input, init))
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) }); await settle()
  expect(text()).toContain('修复回归')
  expect(text()).toContain('ticket read failed')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})
