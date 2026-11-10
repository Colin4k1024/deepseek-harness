// @vitest-environment jsdom
// The Settings groups, drafts, save bars and history.
import { Component, act, useState, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import { Fields, ModelField, SettingsProvider, SourceField, numberOf, useBlockDraft, useSettings } from '../src/client/settings-kit.tsx'
import { expect, it, onTestFinished, vi } from 'vitest'
import { zh } from '../src/client/locales.ts'
import { PanelProvider } from '../src/client/provider.tsx'
import { translator, usePanel, mount, remount, settle, all, find, tab, click, type, text, field, at } from './fixtures/panel.client.tsx'
import { server } from './fixtures/panel-server.client.ts'

usePanel()
const requestUrl = (input: RequestInfo | URL): string => input instanceof URL ? input.href : input instanceof Request ? input.url : input
it('keeps a person\'s own value where another save changed the same setting, and says so', async () => {
  await mount()
  await click(zh['tab.execution'])
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  await type(field<HTMLInputElement>(zh.maxWorkers), '6')
  server.settings = { ...server.settings, executor: { ...server.settings.executor, maxWorkers: 4 } }
  server.revisions.executor = 1
  await click(zh.save)
  expect(document.querySelector('.dt-savebar [role="alert"]')?.textContent).toBe(zh.saveConflict)
  expect(field<HTMLInputElement>(zh.maxWorkers).value).toBe('6')
  await click(zh.save)
  expect(server.settings.executor.maxWorkers).toBe(6)
})

it('saves the Eval block on the Policy settings and the Executor with its execution environment on the Execution settings, each against its own revision', async () => {
  await mount()
  await click(zh['tab.eval'])
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  await click(zh['mode.auto'])
  await type(field<HTMLInputElement>(zh.evalTimeout), '180')
  const [provider, model] = all('.dt-model input')
  await type(provider as HTMLInputElement, 'worker-provider'); await type(model as HTMLInputElement, 'worker-model')
  await click(zh.save)
  expect(server.settings.eval).toEqual({ mode: 'auto', timeoutSeconds: 180, model: { provider: 'worker-provider', model: 'worker-model' } })
  await click(find(zh['promptPart.runtime/work'], '.dt-settings-link'))
  await type(field<HTMLInputElement>(zh.maxWorkers), '5')
  // An empty root uses the default directory, which the field names instead of looking set.
  expect(field<HTMLInputElement>(zh.executionDirectory).closest('.dt-field')!.querySelector('small')?.textContent).toBe('留空时用 /srv/dsh-superagent/work')
  await type(field<HTMLInputElement>(zh.executionDirectory), '/srv/work')
  // A variable picked from the list goes where the caret is in the prepare command.
  const prepare = field<HTMLTextAreaElement>(zh.prepareCommand)
  await type(prepare, 'git worktree add ')
  prepare.setSelectionRange(17, 17)
  await click('可用的环境变量 3')
  expect(all('.dt-table tbody tr').map(row => row.textContent)).toEqual(['DSH_SUPERAGENT_DIR工单的执行目录执行器', 'DSH_SUPERAGENT_NAME工单短名执行器',
    'DSH_SUPERAGENT_GITHUB_REPOSITORY仓库GitHub · 已关闭'])
  await click('DSH_SUPERAGENT_DIR')
  expect(field<HTMLTextAreaElement>(zh.prepareCommand).value).toBe('git worktree add "$DSH_SUPERAGENT_DIR"')
  await type(field<HTMLInputElement>(zh.blockedCommands), 'gh-personal  gh.other')
  const [workerProvider, workerModel, effort] = all('.dt-model input')
  await type(workerProvider as HTMLInputElement, 'worker-provider'); await type(workerModel as HTMLInputElement,
    'worker-model'); await type(effort as HTMLInputElement, 'high')
  expect([...document.querySelectorAll('#dt-worker-model-efforts option')].map(option => option.getAttribute('value'))).toEqual(['high'])
  await click(zh.save)
  expect(server.settings.executor).toEqual({ maxWorkers: 5, directory: '/srv/work',
    prepare: 'git worktree add "$DSH_SUPERAGENT_DIR"', blockedCommands: ['gh-personal', 'gh.other'], model: { provider: 'worker-provider',
      model: 'worker-model', reasoningEffort: 'high' } })
  expect(server.requests.filter(request => request.method === 'PUT').map(request => request.path)).toEqual(['settings/eval',
    'settings/executor'])
})

it('loads configuration history on demand with localized changes, Source field labels and notified Tickets', async () => {
  server.history = { current: 3, policyVersion: 'abcdef1234567890', items: [
    { revision: 3, savedAt: '2026-09-22T00:00:00Z', source: 'sync', policyVersion: 'abcdef1234567890', policyChanged: true,
      notified: [{ id: 't1', title: '修复回归' }], block: 'policy',
      changes: [{ code: 'mode', from: 'off', to: 'observe' }, { code: 'sourceRowsAdded', source: 'feishu', field: 'allowed', items: ['Bob'],
        part: true }, { code: 'sourceField', source: 'feishu', field: 'syncChat', from: '', to: 'oc_team', part: true }] },
    { revision: 2, savedAt: '2026-09-21T00:00:00Z', source: 'panel', policyVersion: 'abcdef1234567890', policyChanged: false,
      notified: [], block: undefined, changes: null },
    { revision: 1, savedAt: '2026-09-20T00:00:00Z', source: 'sync', policyVersion: 'abcdef1234567890', policyChanged: false,
      notified: [], block: 'eval', changes: [] },
  ] }
  await mount()
  await click(zh['tab.eval'])
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  expect(server.requests.some(request => request.path === 'policy/history')).toBe(false)
  await click(zh.configHistory)
  for (const expected of ['第 3 次保存 · 从群同步', '生成策略版本 abcdef12', '接单模式：关闭 → 仅观察', '飞书 · 能发起工作的飞书用户 新增：Bob', '飞书 · 同步来源群：未设置 → oc_team',
    '因策略变更通知了 1 个进行中的工单：修复回归']) expect(text()).toContain(expected)
})

it('shows save and discard only while a settings block has edits, at the end of that block', async () => {
  await mount()
  await click(zh['tab.execution'])
  await click(find(zh['tab.settings'], '.dt-status-head button'))
  expect(all('.dt-savebar')).toHaveLength(0)
  await type(field<HTMLInputElement>(zh.maxWorkers), '6')
  expect(all('.dt-savebar')).toHaveLength(1)
  await type(field<HTMLInputElement>(zh.maxWorkers), '3')
  expect(all('.dt-savebar')).toHaveLength(0)
  await type(field<HTMLInputElement>(zh.maxWorkers), '6')
  await click(zh.discard)
  expect(all('.dt-savebar')).toHaveLength(0)
  // A save submitted from a field keeps the focus there; one from the bar hands it to the block when the bar closes.
  const workers = field<HTMLInputElement>(zh.maxWorkers)
  await type(workers, '5')
  workers.focus()
  await act(async () => { workers.form!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })) }); await settle()
  expect(server.settings.executor.maxWorkers).toBe(5)
  expect(document.activeElement).toBe(workers)
  await type(workers, '4')
  const save = find(zh.save)
  save.focus()
  await click(save)
  expect(all('.dt-savebar')).toHaveLength(0)
  expect(document.activeElement).toBe(workers.form)
  await click(find(zh['term.source'], '.dt-settings-link'))
  expect(all('.dt-savebar')).toHaveLength(0)
  const github = all('.dt-card').find(card => card.querySelector('.dt-card-title')?.textContent === 'GitHub')!
  await type(field<HTMLInputElement>('智能体的 GitHub CLI'), 'gh-other')
  expect(github.lastElementChild!.classList.contains('dt-savebar')).toBe(true)
  // The agent CLI is read by setup, so the bar warns that saving restarts dsh-superagent and interrupts running work.
  expect(github.lastElementChild!.textContent).toContain('2 个进行中的派工会中断')
})

it('gives the focus back to the field that had it once a block\'s save ends', async () => {
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  const render = (disabled: boolean) => act(async () => { root.render(<Fields disabled={disabled}><input aria-label="value" /></Fields>) })
  await render(false)
  const input = container.querySelector('input')!
  input.focus()
  await render(true)
  // jsdom does not blur a control when it becomes disabled; move focus away manually.
  const elsewhere = document.createElement('button'); document.body.append(elsewhere); elsewhere.focus(); elsewhere.remove()
  expect(document.activeElement).toBe(document.body)
  await render(false)
  expect(document.activeElement).toBe(input)
})

it('lists the settings groups stages first, opens each stage\'s group from its status, and returns to the last group opened',
  async () => {
    await mount()
    await click(tab(zh['tab.settings']))
    expect(all('.dt-settings-link').map(link => link.textContent)).toEqual(['来源', '评估', '执行', '提示词', '部署'])
    expect(document.querySelector('.dt-settings-link[aria-current="page"]')?.textContent).toBe(zh['term.source'])
    for (const [tab, group] of [['来源', '来源'], ['评估', '评估'], ['执行', '执行']] as const) {
      await click(find(tab, '[role="tab"]'))
      await click(find(zh['tab.settings'], '.dt-status-head button'))
      expect(document.querySelector('.dt-settings-link[aria-current="page"]')?.textContent).toBe(group)
    }
    // The Trigger stage has no settings, so its status offers none.
    await click(tab(zh['tab.trigger']))
    expect(all('.dt-status-head button').map(button => button.textContent)).not.toContain(zh['tab.settings'])
    await click(tab(zh['tab.settings']))
    expect(document.querySelector('.dt-settings-link[aria-current="page"]')?.textContent).toBe(zh['promptPart.runtime/work'])
    await click(tab(zh['tab.trigger']))
    await remount()
    await click(tab(zh['tab.settings']))
    expect(document.querySelector('.dt-settings-link[aria-current="page"]')?.textContent).toBe(zh['promptPart.runtime/work'])
  })


it('renders every Source field form and reports their edits, including rows, refreshes, and facts', async () => {
  const routes: object[] = []
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  const refreshes: (string[] | Error)[] = [['新群'], [], new Error('lark-cli: token expired')]
  let refreshCall = 0
  const initial = {
    switch: false,
    select: undefined,
    number: 0,
    choose: 'unknown',
    readonly: 'fixed',
    fact: undefined,
    rows: [{ id: 'one', name: 'One' }, ...Array.from({ length: 6 }, (_, index) => ({ id: `r${index}`, name: `R${index}` }))],
    plainRows: [{ id: 'saved', name: 'Saved' }],
  }
  function Harness() {
    const [value, setValue] = useState(initial)
    const edit = (key: keyof typeof initial) => (next: unknown) => { setValue({ ...value, [key]: next }) }
    return <PanelProvider t={translator(zh)} navigate={(route) => { routes.push(route) }}>
      <SourceField field={{ type: 'switch', key: 'switch', label: { zh: '开关', en: 'Switch' } }} value={value.switch} saved={false} fact={undefined}
        labels={{}} onChange={edit('switch')} />
      <SourceField field={{ type: 'select', key: 'select', label: { zh: '选择', en: 'Select' }, options: [{ value: 'a', label: { zh: '甲', en: 'A' } },
        { value: 'b', label: { zh: '乙', en: 'B' } }] }} value={value.select} saved={undefined} fact={undefined} labels={{}}
      onChange={edit('select')} />
      <SourceField field={{ type: 'number', key: 'number', label: { zh: '数量', en: 'Count' }, min: 0, max: 10 }} value={value.number}
        saved={0} fact={undefined} labels={{}} onChange={edit('number')} />
      <SourceField field={{ type: 'text', key: 'choose', label: { zh: '已选', en: 'Chosen' }, choose: { zh: '选择成员', en: 'Choose' } }}
        value={value.choose} saved={'unknown'} fact={undefined} labels={{ known: '已知' }} onChange={edit('choose')} />
      <SourceField field={{ type: 'text', key: 'readonly', label: { zh: '只读', en: 'Read-only' }, readonly: true }} value={value.readonly}
        saved={'fixed'} fact={undefined} labels={{}} onChange={edit('readonly')} />
      <SourceField field={{ type: 'fact', key: 'fact', label: { zh: '事实', en: 'Fact' }, settings: 'deployment' }} value={undefined}
        saved={undefined} fact={{ zh: '部署完成', en: 'Deployed' }} labels={{}} onChange={edit('fact')} />
      <SourceField field={{ type: 'rows', key: 'rows', label: { zh: '行', en: 'Rows' }, hint: { zh: '提示', en: 'Hint' }, columns: [
        { key: 'id', label: { zh: '标识', en: 'ID' }, type: 'text', choose: { zh: '选择行', en: 'Choose row' }, refresh: { zh: '刷新', en: 'Refresh' } },
        { key: 'name', label: { zh: '名称', en: 'Name' }, type: 'text', optional: true },
        { key: 'mode', label: { zh: '模式', en: 'Mode' }, type: 'select', options: [{ value: 'auto', label: { zh: '自动', en: 'Auto' } },
          { value: 'manual', label: { zh: '手动', en: 'Manual' } }] },
      ] }} value={value.rows} saved={[{ id: 'one', name: 'One' }]} fact={undefined}
      labels={{ one: '一号', r0: '零号' }} refreshLabels={async () => {
        const next = refreshes[refreshCall++]!
        return next instanceof Error ? Promise.reject(next) : next
      }} onChange={edit('rows')} />
      <SourceField field={{ type: 'rows', key: 'plainRows', label: { zh: '普通行', en: 'Plain rows' }, columns: [
        { key: 'id', label: { zh: '普通标识', en: 'Plain ID' }, type: 'text' },
        { key: 'name', label: { zh: '普通名称', en: 'Plain name' }, type: 'text' },
      ] }} value={value.plainRows} saved={[{ id: 'saved', name: 'Saved' }]} fact={undefined} labels={{ saved: '已保存' }}
      onChange={edit('plainRows')} />
    </PanelProvider>
  }
  await act(async () => { root.render(<Harness />) }); await settle()

  await click('开关')
  expect([...document.querySelectorAll('[role="switch"]')].at(-1)!.getAttribute('aria-checked')).toBe('true')
  await click('乙')
  const number = field<HTMLInputElement>('数量')
  await type(number, ''); expect(number.value).toBe('')
  await type(number, '7'); expect(number.value).toBe('7')
  expect(numberOf('')).toBeNull(); expect(numberOf(' 4 ')).toBe(4)
  expect(field<HTMLInputElement>('只读').readOnly).toBe(true)
  expect(text()).toContain('部署完成')
  await click(zh['openSettingsOf.deployment'])
  expect(routes).toEqual([{ page: 'settings', query: { group: 'deployment', focus: undefined } }])

  const summary = document.querySelector('.dt-rows-summary')!
  expect(summary.getAttribute('aria-expanded')).toBe('false')
  expect(summary.textContent).toContain('One')
  await click(summary as HTMLElement)
  expect(summary.getAttribute('aria-expanded')).toBe('true')
  expect([...document.querySelectorAll('.dt-rows')].at(-2)!.children.length).toBe(7)
  await click(summary as HTMLElement)
  await click(find('刷新', '.dt-subhead-actions button'))
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe('列表已更新，新增：新群')
  await click(find('刷新', '.dt-subhead-actions button'))
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe(zh.labelsCurrent)
  await click(find('刷新', '.dt-subhead-actions button'))
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe('读取失败，列表保持不变：lark-cli: token expired')

  const plain = document.querySelector('.dt-rows .dt-row-label')!
  expect(plain.textContent).toBe('已保存')
  const plainRows = () => [...document.querySelectorAll('.dt-rows')].at(-1)!
  const plainField = plainRows().closest('.dt-field')!
  await click([...plainField.querySelectorAll<HTMLElement>('.dt-subhead-actions button')].find(button => button.textContent === zh.addRow)!)
  await type(plainRows().querySelector('.dt-rows-item input[aria-label="普通名称"]') as HTMLInputElement, 'Added')
  expect((plainRows().querySelector('.dt-rows-item input[aria-label="普通标识"]') as HTMLInputElement).value).toBe('')
  await click(plainRows().querySelector('.dt-rows-item button')!)
  expect([...plainRows().children].length).toBe(1)
})

it('reports a failed model catalog read and clears a model by clearing every part', async () => {
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  server.down = true
  function ModelHarness() {
    const [value, setValue] = useState<{ provider: string; model: string; reasoningEffort?: string } | null>({ provider: 'other', model: 'other-model',
      reasoningEffort: 'low' })
    return <PanelProvider t={translator(zh)}>
      <ModelField id="test-model" value={value} onChange={(next) => {
        setValue(next ? {
          provider: next.provider, model: next.model,
          ...next.reasoningEffort ? { reasoningEffort: next.reasoningEffort } : {},
        } : null)
      }} />
      <pre aria-label="model value">{JSON.stringify(value)}</pre>
    </PanelProvider>
  }
  await act(async () => { root.render(<ModelHarness />) }); await settle()
  expect(text()).toContain(zh.defaultModelUnavailable)
  expect(text()).toContain(zh.modelCatalogError.replace('{error}', zh.hostNotRunning))

  server.down = false
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    if (!requestUrl(input).endsWith('/models')) return original(input, init)
    const response = await original(input, init)
    const value = JSON.parse(await response.text()) as { default: { reasoningEffort?: string } }
    return Response.json({ ...value, default: { ...value.default, reasoningEffort: 'low' } })
  })
  await act(async () => { root.render(<ModelHarness key="catalog" />) }); await settle()
  expect(text()).toContain(zh.defaultModel.replace('{model}', 'host / default · low'))
  vi.mocked(globalThis.fetch).mockImplementation(original)
  expect([...document.querySelectorAll<HTMLOptionElement>('#test-model-providers option')].map(option => option.value)).toEqual(['worker-provider'])
  const [provider, model, effort] = all('.dt-model input')
  await type(provider as HTMLInputElement, ''); await type(model as HTMLInputElement, ''); await type(effort as HTMLInputElement, '')
  expect(text()).toContain('host / default')
  expect(find('model value', '[aria-label="model value"]')?.textContent).toBe('null')
})

it('refuses a settings hook outside its provider', () => {
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  class Boundary extends Component<{ children: ReactNode }, { message?: string }> {
    override state: { message?: string } = {}
    override render() { return this.state.message ?? this.props.children }
    override componentDidCatch(failure: Error) { this.setState({ message: failure.message }) }
  }
  function Outside() { useSettings(); return null }
  act(() => { root.render(<Boundary><Outside /></Boundary>) })
  expect(container.textContent).toBe('A settings block rendered outside the settings')
})

it('lists Source history under a Source block prefix', async () => {
  server.history = { current: 2, policyVersion: 'abcdef1234567890', items: [{ revision: 2, savedAt: '2026-09-22T00:00:00Z',
    source: 'panel', policyVersion: 'abcdef1234567890', policyChanged: false, notified: [], block: 'source:feishu', changes: [] }] }
  at({ page: 'settings', query: { group: 'source' } })
  await mount()
  await click(zh.configHistory)
  expect(all('.dt-row-title').map(item => item.textContent)).toContain('第 2 次保存 · 管理页')
})

it('says when the configuration history is empty', async () => {
  server.history = { current: 0, policyVersion: 'abcdef1234567890', items: [] }
  at({ page: 'settings', query: { group: 'eval' } })
  await mount()
  await click(zh.configHistory)
  expect(text()).toContain(zh.historyEmpty)
})

it('highlights the save that made a focused Policy version and reports an unrecorded one', async () => {
  server.history = { current: 1, policyVersion: 'abcdef1234567890', items: [{ revision: 1, savedAt: '2026-09-22T00:00:00Z',
    source: 'sync', policyVersion: 'abcdef1234567890', policyChanged: true, notified: [], block: 'policy', changes: [] }] }
  at({ page: 'settings', query: { group: 'eval', version: 'abcdef1234567890' } })
  await mount()
  expect(document.querySelector('.dt-highlight')).not.toBeNull()
  at({ page: 'settings', query: { group: 'eval', version: 'ffffffffffffffff' } })
  await remount()
  expect(text()).toContain(zh.policyVersionUnrecorded.replace('{version}', 'ffffffff'))
})

it('reports a configuration history read failure', async () => {
  at({ page: 'settings', query: { group: 'eval' } })
  await mount()
  server.down = true
  await click(zh.configHistory)
  expect(text()).toContain(zh.hostNotRunning)
})

it('summarizes empty and sparse rows, and an empty fact with its hint', async () => {
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  const rowsField = (value: unknown, columns: { key: string; label: { zh: string; en: string }; type: 'text'; optional?: boolean }[], key: string) => ({
    field: { type: 'rows' as const, key, label: { zh: `行 ${key}`, en: `Rows ${key}` }, columns }, value, saved: [], fact: undefined, labels: {}, onChange: () => {},
  })
  function Harness() {
    return <PanelProvider t={translator(zh)} navigate={() => {}}>
      <SourceField {...rowsField([], [
        { key: 'id', label: { zh: '空标识', en: 'Empty id' }, type: 'text' },
        { key: 'name', label: { zh: '空名称', en: 'Empty name' }, type: 'text', optional: true },
      ], 'empty')} />
      <SourceField {...rowsField([{}], [
        { key: 'id', label: { zh: '稀疏标识', en: 'Sparse id' }, type: 'text' },
        { key: 'name', label: { zh: '稀疏名称', en: 'Sparse name' }, type: 'text', optional: true },
      ], 'sparse')} />
      <SourceField {...rowsField(Array.from({ length: 7 }, () => ({})), [
        { key: 'id', label: { zh: '空白标识', en: 'Blank id' }, type: 'text' },
        { key: 'name', label: { zh: '空白名称', en: 'Blank name' }, type: 'text', optional: true },
      ], 'blank')} />
      <SourceField field={{ type: 'fact', key: 'fact', label: { zh: '空事实', en: 'Empty fact' }, hint: { zh: '事实提示', en: 'Fact hint' },
        settings: 'deployment' }} value={undefined} saved={undefined} fact={undefined} labels={{}} onChange={() => {}} />
    </PanelProvider>
  }
  await act(async () => { root.render(<Harness />) }); await settle()
  expect(all('.dt-count').map(item => item.textContent)).toEqual(['', '1', '7'])
  expect(all('.dt-rows-summary').at(-1)?.textContent).toContain(zh.listSeparator)
  expect(document.querySelectorAll<HTMLInputElement>('.dt-rows-item input')[1]?.value).toBe('')
  expect(text()).toContain(zh.empty)
  expect(all('.dt-info').at(-1)?.getAttribute('aria-label')).toBe('事实提示')
})

it('keeps settings available while a reload fails and keys a draft without a master session', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  let settingsReads = 0
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const address = requestUrl(input)
    if (address.endsWith('/state')) {
      const response = await original(input, init)
      const value = JSON.parse(await response.text()) as Record<string, unknown>
      return Response.json({ ...value, masterId: undefined })
    }
    if (address.endsWith('/settings')) {
      settingsReads++
      if (settingsReads > 1) return new Response(JSON.stringify({ error: 'settings unavailable' }), { status: 500, headers: { 'content-type': 'application/json' } })
      const response = await original(input, init)
      return Response.json(JSON.parse(await response.text()))
    }
    return original(input, init)
  })
  const container = document.createElement('div'); document.body.append(container)
  const root = createRoot(container)
  onTestFinished(() => { act(() => { root.unmount() }); container.remove() })
  function Harness() {
    const settings = useSettings()
    const draft = useBlockDraft('executor', { maxWorkers: 1 }, 0)
    return <>
      <span>{settings.settings.environment.root}</span>
      <button type="button" onClick={() => { draft.edit({ maxWorkers: 2 }) }}>edit draft</button>
      <button type="button" onClick={() => { void settings.reload() }}>reload settings</button>
    </>
  }
  await act(async () => {
    root.render(<PanelProvider t={translator(zh)}><SettingsProvider><Harness /></SettingsProvider></PanelProvider>)
  }); await settle()
  await click('edit draft')
  expect(sessionStorage.getItem('dsh-superagent/draft//executor')).toContain('"maxWorkers":2')
  await click('reload settings')
  expect(text()).toContain('settings unavailable')
  expect(text()).toContain('/srv/dsh-superagent/work')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})
