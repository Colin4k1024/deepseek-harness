// @vitest-environment jsdom
// The Prompts settings.
import { act } from 'react'
import { builtInBase, builtInParts, defaultPrompts } from '../src/app/prompts.ts'
import { expect, it, vi } from 'vitest'
import { zh } from '../src/client/locales.ts'
import { usePanel, mount, remount, settle, all, tab, click, type, text, field, at } from './fixtures/panel.client.tsx'
import { server, promptVersion } from './fixtures/panel-server.client.ts'

usePanel()
it('lists the team sections, then the built-in parts, edits and saves them separately from settings, and resets one section', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  const names = all('.dt-prompt-nav .dt-nav-item > span').map(item => item.textContent)
  expect(names.slice(0, 3)).toEqual(['沟通方式', '工单范围', '交付要求'])
  expect(names.slice(3)).toEqual(Object.keys(builtInParts()).map(key => zh[`promptPart.${key}` as keyof typeof zh]))
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Custom communication rule')
  expect(text()).toContain('1 处修改')
  await click(tab(zh.promptViewFull))
  // The full prompt shows every part, team or built in, as editable, in the order the Session reads them.
  const parts = all('.dt-preview-part').map(part => [part.querySelector('b')?.textContent, part.hasAttribute('data-editable')])
  expect(parts.slice(0, 5)).toEqual([['共享约定', true], ['主 Session 角色', true], ['沟通方式', true], ['工单范围', true], ['交付要求', true]])
  expect(text()).toContain('Custom communication rule')
  await click(all('.dt-preview-part').find(part => part.querySelector('b')?.textContent === '沟通方式')!.querySelector('button')!)
  await click(zh.promptResetSection)
  expect(text()).not.toContain('处修改')
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Second rule')
  await click(zh.save)
  expect(server.prompts.current.config.communication).toBe('Second rule')
  expect(server.requests.some(request => request.path.startsWith('settings/'))).toBe(false)
})

it('keeps a conflicting prompt draft, rebases it onto the newer version and saves on retry', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Mine')
  server.prompts.revision = 1; server.prompts.current = promptVersion(1, { ...defaultPrompts(), scope: 'Theirs' })
  await click(zh.save)
  expect(document.querySelector('.dt-savebar')?.textContent).toContain('提示词已被修改到版本 1。已保留你的草稿')
  await click(zh.save)
  // The other save's section stays: the draft moved onto version 1 section by section, not as a whole.
  expect(server.prompts.current.config).toMatchObject({ communication: 'Mine', scope: 'Theirs' })
})

it('shows a built-in skill with the CLI notes in the full prompt, editable as a built-in part, and adds a team skill but no skill under a built-in name', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewFull))
  await act(async () => { const select = field<HTMLSelectElement>(zh.promptPreviewTarget); select.value = 'skill/github'; select.dispatchEvent(new Event('change', { bubbles: true })) }); await settle()
  expect(text()).toContain('## CLI usage')
  expect(all('.dt-preview-part[data-editable] b').map(name => name.textContent)).toEqual([zh['promptPart.skill/github']])
  await click(tab(zh.promptViewEdit))
  for (const name of ['Bad Name', 'github']) {
    await type(field<HTMLInputElement>(zh.promptAddSkill), name)
    await click(zh.promptAddSkillAction)
    expect(document.querySelector('[role="alert"]')?.textContent).toBe(zh.promptSkillNameInvalid)
  }
  await type(field<HTMLInputElement>(zh.promptAddSkill), 'testing')
  await click(zh.promptAddSkillAction)
  expect(all('.dt-nav-item').map(item => item.textContent)).toContain('dsh-superagent-testing')
})

it('loads all defaults into the draft from the history without saving them', async () => {
  server.prompts.current = promptVersion(0, { ...defaultPrompts(), scope: 'Old scope' })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewHistory))
  const first = `版本 0 · ${zh.promptHistoryInitial} · ${zh['historySource.defaults']}`
  expect(all('.dt-row-title').map(title => title.textContent)).toContain(first)
  await click(zh.promptDefaults)
  expect(text()).toContain(zh.promptDraftFromDefaults)
  expect(server.requests.some(request => request.path === 'prompts' && request.method === 'PUT')).toBe(false)
})

it('keeps the Prompts save bar hidden while a preview loads for an unchanged draft', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  let shown = false
  const watch = new MutationObserver(() => { if (document.querySelector('.dt-savebar')) shown = true })
  watch.observe(document.body, { childList: true, subtree: true })
  await click(tab(zh.promptViewFull))
  await settle()
  watch.disconnect()
  expect(shown).toBe(false)
})

it('marks team sections that differ from the defaults, and shows a draft as a diff against the saved version or the defaults', async () => {
  server.prompts.current = promptVersion(1, { ...defaultPrompts(), delivery: `${defaultPrompts().delivery}\nCommit as the bot account.` })
  server.prompts.revision = 1
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  expect(all('.dt-prompt-nav .dt-nav-item').filter(item => item.querySelector('.dt-changed-tag')).map(item => item.querySelector('span')!.firstChild?.textContent)).toEqual(['交付要求'])
  await type(field<HTMLTextAreaElement>(zh.promptBody), `${defaultPrompts().communication}\nSign as the agent.`)
  await click(tab(zh.promptViewFull))
  // A draft opens against the saved version: only its change shows, and the parts it leaves as they were fold.
  expect(all('.dt-preview-part ins').map(line => line.textContent)).toEqual(['Sign as the agent.\n'])
  expect(all('.dt-preview-part del')).toHaveLength(0)
  expect(text()).toContain(zh.promptSameAsSaved)
  await click(zh.promptCompareDefault)
  expect(all('.dt-preview-part ins').map(line => line.textContent)).toEqual(['Sign as the agent.\n', 'Commit as the bot account.\n'])
})

it('discards a prompt draft by reading the saved version again', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Mine')
  server.prompts.revision = 1; server.prompts.current = promptVersion(1, { ...defaultPrompts(), communication: 'Theirs' })
  await click(zh.discard)
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe('Theirs')
  expect(all('.dt-savebar')).toHaveLength(0)
})

const builtInItem = (name: string) => all('.dt-prompt-nav .dt-nav-item').find(item => item.querySelector('span')!.firstChild?.textContent === name)!
it('edits a built-in part as an override of its built-in text, marks it changed, and resets it to the built-in text', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  const shared = builtInParts()['runtime/shared']
  await click(builtInItem(zh['promptPart.runtime/shared']))
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe(shared.text)
  expect(text()).toContain(zh['promptReadBy.both'])
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Shared, the team way.')
  expect(builtInItem(zh['promptPart.runtime/shared']).querySelector('.dt-changed-tag')?.textContent).toBe(zh.promptChanged)
  await click(zh.save)
  // The override records the built-in text it was edited from.
  expect(server.prompts.current.config.overrides).toEqual({ 'runtime/shared': { text: 'Shared, the team way.', base: builtInBase(shared.text) } })
  // The full prompt reads the override in place of the built-in text, compared with the defaults.
  await click(tab(zh.promptViewFull))
  expect(all('.dt-preview-part ins').map(line => line.textContent)).toContain('Shared, the team way.\n')
  await click(all('.dt-preview-part[data-editable] button')[0]!)
  await click(zh.promptResetSection)
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe(shared.text)
  await click(zh.save)
  expect(server.prompts.current.config).not.toHaveProperty('overrides')
})

it('says which placeholders a template keeps', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(builtInItem(zh['promptPart.runtime/policy']))
  expect(text()).toContain('这是模板：保留占位符 {version}、{mode}、{rules}，由系统填入内容。')
  expect(text()).toContain(zh['promptReadBy.orchestrator'])
})

it('marks an override whose built-in text an upgrade changed, shows how it changed, and keeps the team\'s text or takes the new one', async () => {
  const dispatch = builtInParts()['runtime/dispatch']
  const overrides = { 'runtime/dispatch': { text: '## dispatch\nOur routing.', base: builtInBase('## dispatch\nThe old routing.') } }
  server.prompts.revision = 1; server.prompts.current = promptVersion(1, { ...defaultPrompts(), overrides })
  server.prompts.defaultChanged = [{ part: 'runtime/dispatch', base: overrides['runtime/dispatch'].base, previous: '## dispatch\nThe old routing.' }]
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  expect(all('.dt-prompt-nav .dt-updated-tag').map(tag => tag.closest('.dt-nav-item')!.querySelector('span')!.firstChild?.textContent))
    .toEqual([zh['promptPart.runtime/dispatch']])
  await click(builtInItem(zh['promptPart.runtime/dispatch']))
  // The change of the built-in text, beside the team's text, which stays, in a region named by the mark.
  const box = () => document.querySelector(`section[aria-label="${zh.promptDefaultUpdated}"]`)
  expect(box()!.textContent).toContain('下面是默认内容的变化。')
  expect([...box()!.querySelectorAll('del')].map(line => line.textContent)).toEqual(['The old routing.\n'])
  expect(box()!.querySelectorAll('ins').length).toBeGreaterThan(0)
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe('## dispatch\nOur routing.')
  // Taking the new built-in text drops the override.
  await click(zh.promptTakeDefault)
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe(dispatch.text)
  expect(all('.dt-default-updated')).toHaveLength(0)
  await click(zh.discard)
  await click(builtInItem(zh['promptPart.runtime/dispatch']))
  // Keeping the team's text records the new built-in text as the one it was edited from.
  await click(zh.promptKeepMine)
  expect(all('.dt-default-updated')).toHaveLength(0)
  await click(zh.save)
  expect(server.prompts.current.config.overrides).toEqual({ 'runtime/dispatch': { text: '## dispatch\nOur routing.', base: builtInBase(dispatch.text) } })
})

it('shows the team\'s text against the new default when the default it was edited from is not known, or is another one', async () => {
  const dispatch = builtInParts()['runtime/dispatch']
  // The override was restored from an older version: its base is not the one the changed default was found for.
  const overrides = { 'runtime/dispatch': { text: '## dispatch\nOur routing.', base: 'restored' } }
  server.prompts.revision = 1; server.prompts.current = promptVersion(1, { ...defaultPrompts(), overrides })
  server.prompts.defaultChanged = [{ part: 'runtime/dispatch', base: 'another', previous: '## dispatch\nThe old routing.' }]
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(builtInItem(zh['promptPart.runtime/dispatch']))
  const box = document.querySelector(`section[aria-label="${zh.promptDefaultUpdated}"]`)!
  // It says the lines are the team's text against the new default, not how the default changed.
  expect(box.textContent).toContain('系统没有记录你编辑时的默认内容；下面是你的文本与新默认内容的对比。')
  expect(box.textContent).not.toContain('下面是默认内容的变化。')
  expect([...box.querySelectorAll('del')].map(line => line.textContent)).toEqual(['Our routing.\n'])
  expect([...box.querySelectorAll('ins')].map(line => line.textContent).join('')).toBe(`${dispatch.text.split('\n').slice(1).join('\n')}\n`)
})

it('keeps both sides\' first overrides when a draft moves onto a version saved meanwhile', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(builtInItem(zh['promptPart.runtime/shared']))
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Shared, ours.')
  const theirs = { 'runtime/dispatch': { text: '## dispatch\nTheirs.', base: builtInBase(builtInParts()['runtime/dispatch'].text) } }
  server.prompts.revision = 1; server.prompts.current = promptVersion(1, { ...defaultPrompts(), overrides: theirs })
  await click(zh.save)
  expect(document.querySelector('.dt-savebar')?.textContent).not.toContain('undefined')
  await click(zh.save)
  expect(server.prompts.current.config.overrides).toEqual({ ...theirs,
    'runtime/shared': { text: 'Shared, ours.', base: builtInBase(builtInParts()['runtime/shared'].text) } })
})

it('shows a draft\'s template of a generated part filled, and names a part by its label when a save is refused', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(builtInItem(zh['promptPart.runtime/environment']))
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Where you work:\n{variables}')
  await click(tab(zh.promptViewFull))
  const environment = all('.dt-preview-part').find(part => part.querySelector('b')?.textContent === zh['promptPart.runtime/environment'])!
  expect(environment.querySelector('pre')?.textContent).toBe('Where you work:\n')
  await click(tab(zh.promptViewEdit))
  await click(builtInItem(zh['promptPart.runtime/policy']))
  await type(field<HTMLTextAreaElement>(zh.promptBody), '## Policy\n{version} {mode}')
  await click(zh.save)
  expect(text()).toContain(`「${zh['promptPart.runtime/policy']}」的模板必须保留占位符 {rules}`)
})

it('lists the overrides of parts this version no longer has, which the next save drops', async () => {
  server.prompts.defaultChanged = [{ part: 'runtime/retired', base: 'b', removed: true }]
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  expect(text()).toContain(zh.promptRemovedParts.replace('{parts}', 'runtime/retired'))
})

it('loads a version from the history without its overrides of parts this version no longer has, so it saves', async () => {
  const dispatch = { text: '## dispatch\nOurs.', base: builtInBase(builtInParts()['runtime/dispatch'].text) }
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewHistory))
  // The version restored overrides a part a later version removed.
  const saved = server.prompts.current
  server.prompts.current = promptVersion(0, { ...defaultPrompts(), overrides: { 'runtime/dispatch': dispatch, 'runtime/retired': { text: 'Old.', base: 'b' } } })
  await click(zh.promptRestore)
  server.prompts.current = saved
  await click(zh.save)
  expect(server.prompts.current.config.overrides).toEqual({ 'runtime/dispatch': dispatch })
})


it('names every kind of prompt section a conflicting save changed', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Team, ours.')
  await type(field<HTMLInputElement>(zh.promptAddSkill), 'testing')
  await click(zh.promptAddSkillAction)
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Skill, ours.')
  await click(builtInItem(zh['promptPart.runtime/shared']))
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Shared, ours.')
  server.prompts.revision = 1
  server.prompts.current = promptVersion(1, { ...defaultPrompts(), communication: 'Team, theirs.', skills: { testing: 'Skill, theirs.' },
    overrides: { 'runtime/shared': { text: 'Shared, theirs.', base: builtInBase(builtInParts()['runtime/shared'].text) } } })
  await click(zh.save)
  const bar = document.querySelector('.dt-savebar')?.textContent ?? ''
  for (const expected of ['版本 1', '沟通方式', 'dsh-superagent-testing', zh['promptPart.runtime/shared']]) expect(bar).toContain(expected)
  await click(zh.save)
  expect(server.prompts.current.config).toMatchObject({ communication: 'Team, ours.', skills: { testing: 'Skill, ours.' },
    overrides: { 'runtime/shared': { text: 'Shared, ours.', base: builtInBase(builtInParts()['runtime/shared'].text) } } })
})

it('says a saved prompt is still pending delivery to the running Sessions', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Pending delivery')
  vi.mocked(globalThis.fetch).mockImplementationOnce(async () => new Response(JSON.stringify({ revision: 1, context_delivery: 'pending' }), {
    status: 200, headers: { 'content-type': 'application/json' },
  }))
  await click(zh.save)
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe(zh.promptSavedPending)
})

it('removes a team skill from the draft and returns to the first team section', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLInputElement>(zh.promptAddSkill), 'temporary')
  await click(zh.promptAddSkillAction)
  expect(text()).toContain('dsh-superagent-temporary')
  await click(zh.remove)
  expect(text()).not.toContain('dsh-superagent-temporary')
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe(defaultPrompts().communication)
})

it('reports a failed prompt preview and a failed discard reload', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  server.down = true
  await click(tab(zh.promptViewFull))
  expect(text()).toContain(zh.hostNotRunning)
  await click(tab(zh.promptViewEdit))
  server.down = false
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Draft to discard')
  server.down = true
  await click(zh.discard)
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe(zh.hostNotRunning)
})

it('copies the full prompt and reports the copy', async () => {
  const writeText = vi.fn().mockResolvedValue(undefined)
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewFull))
  const expected = all('.dt-preview-part').map(part => part.querySelector('pre')?.textContent ?? '').join('\n\n')
  await click(zh.promptCopyAll)
  expect(writeText).toHaveBeenCalledWith(expected)
  expect([...document.querySelectorAll('.dt-toast')].at(-1)?.textContent).toBe(zh.copied)
})

it('loads older prompt history and lists the Sessions still using saved versions', async () => {
  server.prompts.sessions = [{ sessionId: 'session-1', revision: 0 }]
  server.prompts.next_offset = 0
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewHistory))
  const older = { ...server.prompts, revisions: [{ ...server.prompts.revisions[0]!, revision: 1,
    savedAt: '2026-09-24T00:00:00Z', source: 'panel', changed: ['communication'] }], next_offset: null }
  vi.mocked(globalThis.fetch).mockImplementationOnce(async () => new Response(JSON.stringify(older), {
    status: 200, headers: { 'content-type': 'application/json' },
  }))
  const calls = vi.mocked(globalThis.fetch).mock.calls.length
  await click(zh.promptOlder)
  expect(vi.mocked(globalThis.fetch).mock.calls.length).toBeGreaterThan(calls)
  const request = vi.mocked(globalThis.fetch).mock.calls.at(-1)?.[0] as URL | string | undefined
  expect(request instanceof URL ? request.href : (request ?? '')).toContain('prompts?offset=0')
  expect(all('.dt-row-title').map(title => title.textContent)).toHaveLength(2)
  expect(text()).toContain(zh.promptSessionItem.replace('{session}', 'session-1').replace('{revision}', '0'))
})

it('restores a stored draft whose selected section no longer exists', async () => {
  sessionStorage.setItem('dsh-superagent/prompts/master', JSON.stringify({ base: defaultPrompts(), value: defaultPrompts(), revision: 0,
    meta: { selection: 'removed-section' } }))
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  expect(field<HTMLTextAreaElement>(zh.promptBody).value).toBe(defaultPrompts().communication)
  expect(text()).toContain('未保存 · 基于版本 0 · 0 处修改')
})

it('says a built-in skill is read by the skill itself', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(builtInItem(zh['promptPart.skill/github']))
  expect(text()).toContain(zh['promptReadBy.skill'])
})

it('previews a team skill as an editable team section', async () => {
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLInputElement>(zh.promptAddSkill), 'temporary')
  await click(zh.promptAddSkillAction)
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Temporary skill text')
  await click(tab(zh.promptViewFull))
  await act(async () => { const select = field<HTMLSelectElement>(zh.promptPreviewTarget); select.value = 'skill/temporary'; select.dispatchEvent(new Event('change', { bubbles: true })) }); await settle()
  const section = all('.dt-preview-part').find(part => part.querySelector('b')?.textContent === 'dsh-superagent-temporary')!
  expect(section.getAttribute('data-editable')).toBe('true')
  expect(section.querySelector('pre')?.textContent).toContain('Temporary skill text')
})

it('names a conflicting team skill by its skill path', async () => {
  server.prompts.current = promptVersion(0, { ...defaultPrompts(), skills: { testing: 'Base skill text' } })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(all('.dt-nav-item').find(item => item.textContent?.includes('dsh-superagent-testing'))!)
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Mine')
  server.prompts.revision = 1
  server.prompts.current = promptVersion(1, { ...defaultPrompts(), skills: { testing: 'Theirs' } })
  await click(zh.save)
  const bar = document.querySelector('.dt-savebar')?.textContent ?? ''
  expect(bar).toContain('你也改过')
  expect(bar).toContain('dsh-superagent-testing')
})

it('drops stale preview responses that finish after the page remounts', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  let first = true
  let release!: () => void
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = input instanceof Request ? input.url : input instanceof URL ? input.href : input
    if (!url.includes('/prompts/preview')) return original(input, init)
    if (!first) return original(input, init)
    first = false
    return new Promise((resolve) => {
      release = () => { resolve(original(input, init)) }
    })
  })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewFull))
  await remount()
  release()
  await settle()
  expect(text()).not.toContain(zh.hostNotRunning)
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('drops stale preview failures that finish after the page remounts', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  let first = true
  let rejectPreview!: (failure?: unknown) => void
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = input instanceof Request ? input.url : input instanceof URL ? input.href : input
    if (!url.includes('/prompts/preview')) return original(input, init)
    if (!first) return original(input, init)
    first = false
    return new Promise((_resolve, reject) => { rejectPreview = reject })
  })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await click(tab(zh.promptViewFull))
  await remount()
  rejectPreview(new Error('stale preview failed'))
  await settle()
  expect(text()).not.toContain('stale preview failed')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})

it('renders a missing skill preview as empty and a generated part without a built-in entry as read-only', async () => {
  const original = vi.mocked(globalThis.fetch).getMockImplementation()!
  vi.mocked(globalThis.fetch).mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = input instanceof Request ? input.url : input instanceof URL ? input.href : input
    if (!url.includes('/prompts/preview')) return original(input, init)
    const response = await original(input, init)
    const value = JSON.parse(await response.text()) as { parts: { orchestrator: unknown[] }; skills: Record<string, string> }
    const { temporary: _temporary, ...skills } = value.skills
    return Response.json({ ...value, skills, parts: { ...value.parts, orchestrator: [...value.parts.orchestrator,
      { name: 'runtime/extra', kind: 'generated', text: 'Generated without a built-in part' }] } })
  })
  at({ page: 'settings', query: { group: 'prompts' } })
  await mount()
  await type(field<HTMLInputElement>(zh.promptAddSkill), 'temporary')
  await click(zh.promptAddSkillAction)
  await type(field<HTMLTextAreaElement>(zh.promptBody), 'Temporary skill text')
  await click(tab(zh.promptViewFull))
  const generated = all('.dt-preview-part').find(part => part.querySelector('b')?.textContent === 'runtime/extra')!
  expect(generated.getAttribute('data-editable')).toBeNull()
  expect(generated.textContent).toContain(zh.promptBuiltInFolded)
  await act(async () => {
    const select = field<HTMLSelectElement>(zh.promptPreviewTarget)
    const option = document.createElement('option'); option.value = 'skill/temporary'; option.textContent = 'dsh-superagent-temporary'; select.append(option)
    select.value = 'skill/temporary'; select.dispatchEvent(new Event('change', { bubbles: true }))
  }); await settle()
  const skill = all('.dt-preview-part').find(part => part.querySelector('b')?.textContent === 'dsh-superagent-temporary')!
  expect(skill.querySelector('pre')?.textContent ?? '').toBe('')
  vi.mocked(globalThis.fetch).mockImplementation(original)
})
