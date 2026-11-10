// @vitest-environment jsdom
// Panel search, glossary, route persistence, and host-down fallback.
import { act } from 'react'
import { expect, it, vi } from 'vitest'
import { en, zh } from '../src/client/locales.ts'
import { usePanel, mount, remount, settle, all, click, type, text, field, tab, at } from './fixtures/panel.client.tsx'
import { server, found, threadSummary, ticketSummary } from './fixtures/panel-server.client.ts'

usePanel()
it('states why a filtered search result has no Thread instead of opening an empty one', async () => {
  server.search = { items: [found('e9', '2026-09-20T00:00:00Z', { source: 'github', actor: { source: 'github', id: 'someone' },
    preview: 'issue text', threads: ['github:7'],
    filtered: 'source_disabled' })], threads: [] }
  await mount()
  await type(field<HTMLInputElement>(zh.searchLabel), 'issue')
  await act(async () => { field<HTMLInputElement>(zh.searchLabel).closest('form')!.requestSubmit() }); await settle()
  expect(text()).toContain('被来源过滤：来源已停用')
  expect(all('.dt-search-results button').some(button => button.textContent === 'issue text')).toBe(false)
})

it('opens a search result older than the newest page around its Trigger', async () => {
  server.search = { items: [found('e1', '2026-09-20T00:00:00Z')], threads: [] }
  await mount()
  await type(field<HTMLInputElement>(zh.searchLabel), 'message')
  await act(async () => { field<HTMLInputElement>(zh.searchLabel).closest('form')!.requestSubmit() }); await settle()
  expect(text()).toContain('找到 0 个工单、0 个线程、1 条触发')
  await click(all('.dt-search-results button').find(button => button.textContent === 'message e1')!)
  expect(server.requests.some(request => request.path.includes('around=e1'))).toBe(true)
  expect(document.querySelector('.dt-highlight')?.textContent).toContain('message e1')
})

it('explains what can be searched when nothing matches', async () => {
  await mount()
  await type(field<HTMLInputElement>(zh.searchLabel), 'nothing')
  await act(async () => { field<HTMLInputElement>(zh.searchLabel).closest('form')!.requestSubmit() }); await settle()
  expect(text()).toContain('没有找到。可以输入触发文字')
})

it('returns to the exact page after a remount of the main panel', async () => {
  await mount()
  await click('修复回归')
  await remount()
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('修复回归')
})

it('shows domain terms in the interface language only and lists both languages in the (?) glossary', async () => {
  await mount(en)
  expect(all('[role="tab"]').map(tab => tab.textContent)).toEqual(['Overview', 'Source', 'Trigger', 'Eval', 'Execution', 'Settings'])
  await click('Glossary')
  const dialog = document.querySelector('[role="dialog"]')!
  const row = [...dialog.querySelectorAll('tr')].find(item => item.textContent?.startsWith('Ticket'))!
  expect([...row.querySelectorAll('td')].map(cell => cell.textContent)).toEqual(['Ticket', '工单', 'An authorized unit of work'])
  expect(dialog.textContent).toContain('Accept 接单')
  // Terms are grouped the way the tabs follow the flow, after the roles that act in it.
  expect([...dialog.querySelectorAll('th.dt-glossary-group')].map(cell => cell.textContent)).toEqual(['Roles', 'Source', 'Trigger', 'Eval',
    'Execution'])
  expect([...dialog.querySelectorAll('tr')].find(item => item.textContent?.startsWith('Main Session'))!.textContent).toContain('prompts call it the orchestrator')
  await click(document.querySelector<HTMLElement>('.dt-modal-close')!)
  expect(document.querySelector('[role="dialog"]')).toBeNull()
})

it('says dsh-superagent is not answering when its routes are gone, as while it restarts', async () => {
  server.down = true
  await mount()
  expect(text()).toContain(zh.hostNotRunning)
  expect(text()).not.toContain('404')
})

it('closes search results on Escape and on a click outside the search', async () => {
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  const submit = async () => { await type(search,
    'nothing'); await act(async () => { search.closest('form')!.requestSubmit() }); await settle() }
  await submit()
  expect(document.querySelector('.dt-search-results')).not.toBeNull()
  await act(async () => { search.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true })) })
  expect(document.querySelector('.dt-search-results')).not.toBeNull()
  await act(async () => { search.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })) })
  expect(document.querySelector('.dt-search-results')).not.toBeNull()
  await act(async () => { search.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })) })
  expect(document.querySelector('.dt-search-results')).toBeNull()
  await submit()
  await act(async () => { document.querySelector('main')!.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })) })
  expect(document.querySelector('.dt-search-results')).toBeNull()
})

it("caps a large search and shows an ended Ticket, an empty Trigger, and a Trigger's Thread", async () => {
  server.setup = { ...server.setup, accounts: null }
  const fallback = globalThis.fetch
  vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
    const query = new URL(url, 'http://127.0.0.1:3082').searchParams.get('q')
    if (query === 'none') return Response.json({ tickets: [], threads: [], items: [] })
    if (query === 'all') return Response.json({
      tickets: [ticketSummary({ title: '旧工单' }, { stage: 'ended' })],
      threads: [],
      items: [
        { ...found('empty', '2026-09-20T00:00:00Z', { preview: 'none', threadTitle: '相关线程' }), preview: '' },
        ...Array.from({ length: 20 }, (_, index) => found(`e${index}`, '2026-09-21T00:00:00Z')),
      ],
    })
    return fallback(url, init)
  })
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  await type(search, 'none')
  await act(async () => { search.closest('form')!.requestSubmit() }); await settle()
  expect(text()).toContain(zh.searchNoneGithub)
  await type(search, 'all')
  await act(async () => { search.closest('form')!.requestSubmit() }); await settle()
  expect(text()).toContain(zh.searchFoundCapped.replace('{tickets}', '1').replace('{threads}', '0').replace('{items}', '21'))
  expect(text()).toContain(zh['ticket.ended'])
  expect(all('.dt-search-results .dt-row-title').some(title => title.textContent === '—')).toBe(true)
  expect(text()).toContain(zh.inThread.replace('{thread}', '相关线程'))
})

it('ignores a failed search response once a newer search has answered', async () => {
  let failOld = () => {}
  server.searching = query => query === 'old'
    ? new Promise<void>((_, reject) => { failOld = () => { reject(new Error('old search failed')) } })
    : Promise.resolve()
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  await type(search, 'old')
  await act(async () => { search.closest('form')!.requestSubmit() })
  await type(search, 'new')
  await act(async () => { search.closest('form')!.requestSubmit() }); await settle()
  await act(async () => { failOld() }); await settle()
  expect(document.querySelector('.dt-toast')).toBeNull()
})

it('returns to the overview when a deep link has no back stack', async () => {
  at({ page: 'execution', id: 't1' })
  await mount()
  await click(document.querySelector<HTMLElement>('.dt-page-head button')!)
  expect(tab(zh['tab.overview']).getAttribute('aria-selected')).toBe('true')
})

it('shows the answer to the latest search only', async () => {
  server.search = { items: [found('e1', '2026-09-20T00:00:00Z', { preview: 'old message' }), found('e2', '2026-09-21T00:00:00Z',
    { preview: 'new message' })], threads: [] }
  let release = () => {}
  server.searching = async (query) => { if (query === 'old') await new Promise<void>((resolve) => { release = resolve }) }
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  for (const query of ['old', 'new']) { await type(search,
    query); await act(async () => { search.closest('form')!.requestSubmit() }); await settle() }
  expect(all('.dt-search-results .dt-row-title').map(title => title.textContent)).toEqual(['new message'])
  await act(async () => { release() }); await settle()
  expect(all('.dt-search-results .dt-row-title').map(title => title.textContent)).toEqual(['new message'])
})

it('opens Ticket and Thread search results', async () => {
  server.search.threads = [threadSummary()]
  const fallback = globalThis.fetch
  vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
    if (new URL(url, 'http://127.0.0.1:3082').searchParams.get('q') === 'ticket')
      return Response.json({ tickets: [ticketSummary()], threads: [], items: [] })
    return fallback(url, init)
  })
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  const submit = async (query: string) => {
    await type(search, query)
    await act(async () => { search.closest('form')!.requestSubmit() })
    await settle()
  }
  await submit('ticket')
  await click(all('.dt-search-results button').find(button => button.textContent === '修复回归')!)
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('修复回归')
  await submit('thread')
  await click(all('.dt-search-results button').find(button => button.textContent === '这个文案好奇怪')!)
  expect(document.querySelector('.dt-page-title h2')?.textContent).toBe('这个文案好奇怪')
})

it('clears an empty search submission and reports a failed search', async () => {
  const fallback = globalThis.fetch
  vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
    if (new URL(url, 'http://127.0.0.1:3082').searchParams.get('q') === 'broken')
      return new Response(null, { status: 500 })
    return fallback(url, init)
  })
  await mount()
  const search = field<HTMLInputElement>(zh.searchLabel)
  await type(search, 'broken')
  await act(async () => { search.closest('form')!.requestSubmit() })
  await settle()
  expect(document.querySelector('.dt-toast')?.textContent).toContain(zh.serverError.replace('{status}', '500'))
  await type(search, '')
  await act(async () => { search.closest('form')!.requestSubmit() })
  await settle()
  expect(document.querySelector('.dt-search-results')).toBeNull()
})

it('words a failed state request by its HTTP status', async () => {
  vi.stubGlobal('fetch', async () => new Response(null, { status: 502 }))
  await mount()
  expect(text()).toContain(zh.serverError.replace('{status}', '502'))
  vi.stubGlobal('fetch', async () => new Response('<html>Bad request</html>', { status: 400 }))
  await remount()
  expect(text()).toContain(zh.requestFailed.replace('{status}', '400'))
})
