// @vitest-environment jsdom
// The Trigger status and a Thread.
import { act } from 'react'
import { expect, it, onTestFinished, vi } from 'vitest'
import { en, zh } from '../src/client/locales.ts'
import { usePanel, mount, remount, settle, all, find, click, text, at } from './fixtures/panel.client.tsx'
import { server, trigger, threadSummary, ticketSummary, acceptDecision } from './fixtures/panel-server.client.ts'

usePanel()
it('walks a Thread timeline newest first with dispositions as tags and its Decisions, requeues a filtered Trigger, pages backwards and returns', async () => {
  await mount()
  await click(zh['tab.trigger'])
  expect(document.querySelector('.dt-ticket .dt-stage')?.textContent).toBe(zh.threadSubscribed)
  expect(text()).toContain('扇出给 修复回归')
  await click('这个文案好奇怪')
  expect(document.querySelector('.dt-page-head .dt-btn')?.textContent).toBe(zh['term.trigger'])
  expect(text()).not.toContain('扇出到 1 个工单')
  expect(text()).toContain('已过滤：发送者不在这个来源的名单里')
  expect(text()).toContain(zh['term.correlation'])
  expect(all('.dt-event .dt-event-text').map(element => element.textContent)).toEqual(['message e4', 'message e3'])
  await click(zh.requeue)
  expect(server.requests.some(request => request.path === 'triggers/e4/requeue')).toBe(true)
  expect(text()).toContain(zh.dispositionQueued)
  await click(zh.loadEarlier)
  expect(all('.dt-event .dt-event-text').map(element => element.textContent)).toEqual(['message e4', 'message e3', 'message e1'])
  await click(document.querySelector<HTMLElement>('.dt-page-head .dt-btn')!)
  expect(find(zh['threadFilter.subscribed'], '.dt-pill').getAttribute('aria-pressed')).toBe('true')
})

it('offers to send a person\'s message that reached a ticket to Eval, for one its worker neither did nor handed back', async () => {
  await mount()
  await click(zh['tab.trigger'])
  await click('这个文案好奇怪')
  const event = all('.dt-event').find(element => element.textContent?.includes('message e3'))!
  await click([...event.querySelectorAll<HTMLElement>('button')].find(button => button.textContent === zh.requeue)!)
  expect(server.requests.some(request => request.path === 'triggers/e3/requeue' && request.method === 'POST')).toBe(true)
})

it('filters the Thread list by activity, and unsubscribes from a Thread page after confirmation', async () => {
  await mount()
  await click(zh['tab.trigger'])
  expect(find(zh['threadFilter.subscribed'], '.dt-pill').getAttribute('aria-pressed')).toBe('true')
  await click(zh['threadFilter.recent'])
  expect(server.requests.some(request => request.path === 'threads?filter=recent&limit=20')).toBe(true)
  await click(zh['threadFilter.subscribed'])
  await settle()
  expect(server.requests.some(request => request.path === 'threads?filter=subscribed&limit=20')).toBe(true)
  expect(document.querySelector('select[aria-label="来源"]')).toBeNull()
  await click('这个文案好奇怪')
  await click(zh.unsubscribe)
  await click(zh.confirm)
  expect(server.requests.some(request => request.path === 'threads/feishu:oc_1:om_root/unsubscribe')).toBe(true)
})

it('shows an empty Thread list and loads the next page of Threads', async () => {
  server.threads = []
  at({ page: 'trigger' })
  await mount()
  expect(text()).toContain(zh.threadsNone)
  server.threads = [threadSummary()]
  server.nextOffset = 20
  await remount()
  await click(zh.loadMore)
  expect(server.requests.some(request => request.path === 'threads?filter=subscribed&limit=40')).toBe(true)
})

it('folds consecutive automated messages from one sender on a Thread, showing the newest with its text', async () => {
  const bot = (id: string, at: string) => trigger(id, at, { actor: { source: 'github', id: '' }, human: false,
    text: `CI of PR #42 at abc1234: 3 checks, 0 failed, ${id === 'b3' ? 0 : 1} running.`, preview: `CI ${id}` })
  server.threadPages = () => ({ ...threadSummary(), tickets: [], correlated: [], decisions: [], earlier: null, context: [],
    timeline: [bot('b1', '2026-09-23T00:00:00Z'), bot('b2', '2026-09-23T00:01:00Z'), trigger('h1', '2026-09-23T00:02:00Z'), bot('b3',
      '2026-09-23T00:03:00Z'), bot('b4', '2026-09-23T00:04:00Z')] })
  await mount()
  await click(zh['tab.trigger'])
  await click('这个文案好奇怪')
  expect(all('.dt-event .dt-event-text').map(element => element.textContent)).toEqual(['CI of PR #42 at abc1234: 3 checks, 0 failed, 1 running.', 'message h1', 'CI of PR #42 at abc1234: 3 checks, 0 failed, 1 running.'])
  const folds = all('.dt-event').map(event => event.querySelector('button.dt-textlink[aria-expanded]')?.textContent ?? '')
  expect(folds).toEqual(['同一发送者之前还有 1 条自动消息', '', '同一发送者之前还有 1 条自动消息'])
  await click(find('同一发送者之前还有 1 条自动消息'))
  expect(text()).toContain('CI b3')
})

it('reads the page around a link\'s target once, also for a Trigger the Thread does not have, while it polls the newest page', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
  onTestFinished(() => { vi.useRealTimers() })
  at({ page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: 'gone' } })
  await mount()
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) }); await settle()
  expect(server.requests.filter(request => request.path.startsWith('threads/')).map(request => request.path)).toEqual([
    'threads/feishu:oc_1:om_root?limit=50', 'threads/feishu:oc_1:om_root?limit=50&around=gone', 'threads/feishu:oc_1:om_root?limit=50'])
})

it('shows a follow-up delivered to its Ticket without an Eval, a hand-back with why, and a Ticket that follows the Thread for context', async () => {
  server.threadPages = () => ({ ...threadSummary(), tickets: [ticketSummary(), ticketSummary({ id: 't2', title: '相关工单' })],
    correlated: [], decisions: [], earlier: null, context: ['t2'], url: undefined,
    timeline: [trigger('f1', '2026-09-23T00:00:00Z', { disposition: { delivered: true, fanout: ['t1'] } }),
      trigger('f2', '2026-09-23T00:01:00Z', { disposition: { queued: true, fanout: ['t1'], handbacks: [{ ticketId: 't1', title: '修复回归',
        reason: 'Asks for a new page', at: '2026-09-23T00:02:00Z' }] } })] })
  at({ page: 'trigger', id: 'feishu:oc_1:om_root' })
  await mount()
  const events = () => all('.dt-event').map(event => [event.querySelector('.dt-event-text')?.textContent,
    [...event.querySelectorAll('.dt-tag')].map(tag => tag.textContent), event.querySelector('.dt-hint')?.textContent])
  expect(events()).toEqual([['message f2', [zh.dispositionQueued], '“修复回归”的执行器把这条触发交回评估：Asks for a new page'],
    ['message f1', [zh.dispositionDelivered], undefined]])
  expect(document.querySelector(`.dt-tag[title="${zh.dispositionDeliveredHint}"]`)).not.toBeNull()
  expect(all('.dt-list .dt-ticket').map(row => [row.querySelector('.dt-open')?.textContent, row.querySelector('.dt-tag')?.textContent]))
    .toEqual([['修复回归', undefined], ['相关工单', zh.contextOnly]])
  await remount(en)
  expect(events()[1]).toEqual(['message f1', [en.dispositionDelivered], undefined])
  expect(events()[0]![2]).toBe('The Executor of “修复回归” handed it back for an Eval: Asks for a new page')
})

/** Simulate a Thread with `count` Triggers 1 minute apart, paged 50 at a time via `before` or `around` cursors. */
function busyThread(count: number) {
  const minute = (n: number) => new Date(Date.UTC(2026, 8, 1) + n * 60_000).toISOString()
  const thread = { triggers: Array.from({ length: count }, (_, n) => trigger(`m${n}`, minute(n))),
    add: (more: number) => { const from = thread.triggers.length; thread.triggers.push(...Array.from({ length: more },
      (_, n) => trigger(`m${from + n}`, minute(from + n)))) } }
  server.threadPages = (_key, { before, around }) => {
    const end = around ? thread.triggers.find(item => item.id === around)?.at : undefined
    const kept = thread.triggers.filter(item => end ? item.at <= end : !before || item.at < before)
    const timeline = kept.slice(-50)
    return { ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], url: undefined, timeline,
      earlier: kept.length > 50 ? timeline[0]!.at : null }
  }
  return thread
}
/** The Triggers the timeline shows, oldest first, by their number. */
const shown = () => all('.dt-event .dt-event-text').map(item => Number(item.textContent.replace('message m', ''))).reverse()
const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, n) => from + n)

it('reads back the Triggers between the newest page and the earlier ones it shows, once more arrive than a page holds', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
  onTestFinished(() => { vi.useRealTimers() })
  const thread = busyThread(120)
  at({ page: 'trigger', id: 'feishu:oc_1:om_root' })
  await mount()
  expect(shown()).toEqual(range(70, 119))
  await click(zh.loadEarlier)
  expect(shown()).toEqual(range(20, 119))
  // Fewer new Triggers than a page holds still join the earlier ones; more leave a gap, which is read back.
  thread.add(10)
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) }); await settle()
  expect(shown()).toEqual(range(20, 129))
  thread.add(60)
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) }); await settle()
  expect(shown()).toEqual(range(20, 189))
  await click(zh.loadEarlier)
  expect(shown()).toEqual(range(0, 189))
})

it('shows a link\'s target far back with every Trigger after it', async () => {
  busyThread(120)
  at({ page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: 'm10' } })
  await mount()
  expect(shown()).toEqual(range(0, 119))
  expect(document.querySelector('.dt-highlight .dt-event-text')?.textContent).toBe('message m10')
  expect(all('button').some(button => button.textContent === zh.loadEarlier)).toBe(false)
})

it("opens a Ticket named by a Thread decision and expands the Thread's Decisions", async () => {
  const decision = { ...acceptDecision(), ticketId: 't2', ticketTitle: '新工单' }
  server.threadPages = () => ({ ...threadSummary(), tickets: [], correlated: [], decisions: [decision], context: [], earlier: null,
    timeline: [trigger('o1', '2026-09-23T00:00:00Z', { disposition: { decision } })] })
  server.ticketDetails.t2 = { ...server.ticketDetails.t1!, id: 't2', title: '新工单' }
  at({ page: 'trigger', id: 'feishu:oc_1:om_root' })
  await mount()
  await click(document.querySelector<HTMLElement>('[data-disclosure-row]')!)
  expect(document.querySelector('.dt-disclosure')?.getAttribute('data-open')).toBe('true')
  await click(zh.openTicket)
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('新工单')
})

it('reports a missing Thread when a correlation link opens one the server no longer has', async () => {
  at({ page: 'trigger', id: 'feishu:oc_1:om_root' })
  await mount()
  await click('PR 42')
  expect(text()).toContain(zh.threadNotFound)
})

it('reports when the Trigger page needed to fill a deep-linked gap is gone', async () => {
  server.threadPages = (_key, query) => {
    if (query.around === 'gone')
      return { ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], earlier: null,
        timeline: [trigger('old', '2026-09-20T00:00:00Z')] }
    if (query.before) return undefined
    return { ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], earlier: 'cursor-1',
      timeline: [trigger('new', '2026-09-23T00:00:00Z')] }
  }
  at({ page: 'trigger', id: 'feishu:oc_1:om_root', query: { trigger: 'gone' } })
  await mount()
  expect(text()).toContain('Thread not found')
})

it('marks a queued Trigger and fanout to Tickets it does not list', async () => {
  server.threadPages = () => ({ ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], earlier: null,
    url: undefined, timeline: [
      trigger('queued', '2026-09-23T00:00:00Z', { disposition: { queued: true } }),
      trigger('fanout', '2026-09-23T00:01:00Z', { disposition: { fanout: ['other-ticket'] } }),
    ] })
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  expect(all('.dt-tag').map(tag => tag.textContent)).toEqual([zh.dispositionFanout.replace('{count}', '1'), zh.dispositionQueued])
})

it('shows an empty Thread without an original link, and its live evaluation and polling problems', async () => {
  server.threadPages = () => ({
    ...threadSummary({ evaluating: { triggers: 2, strength: 'explicit', deadline: '2026-09-23T00:10:00Z' }, pollError: 'socket closed' }),
    tickets: [], correlated: [], decisions: [], context: [], earlier: null, url: undefined, timeline: [],
  })
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  expect(text()).toContain(zh.noTriggers)
  expect(text()).toContain('评估进行中：2 条触发，明确，')
  expect(text()).toContain(zh.pollFailed.replace('{error}', 'socket closed'))
  expect(all('button').some(button => button.textContent === zh.openOriginal)).toBe(false)
  expect(all('button').some(button => button.textContent === zh.unsubscribe)).toBe(false)
})

it('reads a deep-linked Trigger into the newest page', async () => {
  server.threadPages = (_key, { around, before }) => around === 'old' || before ? { ...threadSummary(), tickets: [], correlated: [], decisions: [],
    context: [], earlier: null, url: undefined, timeline: [trigger('old', '2026-09-20T00:00:00Z')] }
    : { ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], earlier: 'cursor', url: undefined,
      timeline: [trigger('new', '2026-09-23T00:00:00Z')] }
  at({ page: 'trigger', id: threadSummary().key, query: { trigger: 'old' } })
  await mount()
  expect(all('.dt-event .dt-event-text').map(item => item.textContent)).toEqual(['message new', 'message old'])
  expect(document.querySelector('.dt-highlight .dt-event-text')?.textContent).toBe('message old')
})

it('reports a Trigger already queued for Eval', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (url: string | URL, init?: RequestInit) => String(url).includes('/triggers/')
    ? new Response(JSON.stringify({ queued: false }), { status: 200, headers: { 'content-type': 'application/json' } })
    : original(url, init))
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  await click(zh.requeue)
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe(zh.requeueAlready)
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('says an unsubscribe receipt is still pending', async () => {
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  await click(zh.unsubscribe)
  vi.mocked(globalThis.fetch).mockImplementationOnce(async () => new Response(JSON.stringify({ removed: true,
    thread: threadSummary().key, context_delivery: 'pending' }), { status: 200, headers: { 'content-type': 'application/json' } }))
  await click(zh.confirm)
  expect([...document.querySelectorAll('.dt-toast')].map(item => item.textContent)).toContain(zh.unsubscribePending)
})

it('marks a Trigger being evaluated and reads its preview when it has no text', async () => {
  server.threadPages = () => ({ ...threadSummary(), tickets: [], correlated: [], decisions: [], context: [], earlier: null,
    timeline: [trigger('evaluating', '2026-09-23T00:00:00Z', { text: undefined, disposition: { evaluating: true } })] })
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  expect(all('.dt-tag').map(tag => tag.textContent)).toEqual([zh.dispositionEvaluating])
  expect(document.querySelector('.dt-event-text')?.textContent).toBe('message evaluating')
})

it('reports a failed read of a deep-linked Trigger', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (url: string | URL, init?: RequestInit) => String(url).includes('around=old')
    ? new Response(JSON.stringify({ error: 'deep link unavailable' }), { status: 500, headers: { 'content-type': 'application/json' } })
    : original(url, init))
  at({ page: 'trigger', id: threadSummary().key, query: { trigger: 'old' } })
  await mount()
  expect(text()).toContain('deep link unavailable')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('reports a failed read of the Triggers missing from a gap', async () => {
  busyThread(120)
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (url: string | URL, init?: RequestInit) => String(url).includes('before=')
    ? new Response(JSON.stringify({ error: 'gap unavailable' }), { status: 500, headers: { 'content-type': 'application/json' } })
    : original(url, init))
  at({ page: 'trigger', id: threadSummary().key, query: { trigger: 'm10' } })
  await mount()
  expect(text()).toContain('gap unavailable')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('keeps the Thread page and reports a later polling failure', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
  onTestFinished(() => { vi.useRealTimers() })
  at({ page: 'trigger', id: threadSummary().key })
  await mount()
  server.down = true
  await act(async () => { await vi.advanceTimersByTimeAsync(5000) }); await settle()
  expect(text()).toContain(zh.hostNotRunning)
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('这个文案好奇怪')
})
