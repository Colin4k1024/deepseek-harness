// @vitest-environment jsdom
import { act, createElement } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { SettingsSchema } from '../src/policy.ts'
import { PanelProvider } from '../src/client/provider.tsx'
import { draftReducer, merge3, saveBarText, useDraft, SaveBar, type DraftAction, type DraftState } from '../src/client/draft.tsx'
import { ApiError } from '../src/client/api.ts'
import type { T } from '../src/client/text.ts'

const merged = (base: unknown, mine: unknown, theirs: unknown) => { const conflicts: string[][] = []; return { value: merge3(base, mine,
  theirs, conflicts), conflicts } }

it('takes each key from the side that changed it, through nested records', () => {
  const base = { mode: 'observe', model: { provider: 'a', model: 'x' }, skills: { one: '1', two: '2' } }
  expect(merged(base, { ...base, mode: 'auto', skills: { one: '1!', two: '2' } }, { ...base, model: { provider: 'b', model: 'x' },
    skills: { one: '1', two: '2?' } }))
    .toEqual({ value: { mode: 'auto', model: { provider: 'b', model: 'x' }, skills: { one: '1!', two: '2?' } }, conflicts: [] })
})

it('keeps mine where both sides changed a key, and names it by its path', () => {
  const base = { eval: { timeoutSeconds: 120, mode: 'off' } }
  expect(merged(base, { eval: { timeoutSeconds: 60, mode: 'off' } }, { eval: { timeoutSeconds: 300, mode: 'auto' } }))
    .toEqual({ value: { eval: { timeoutSeconds: 60, mode: 'auto' } }, conflicts: [['eval', 'timeoutSeconds']] })
})

it('removes a key one side removed, unless the other side changed it', () => {
  const base = { skills: { one: '1', two: '2', three: '3' } }
  expect(merged(base, { skills: { two: '2', three: '3!' } }, { skills: { one: '1', two: '2' } }))
    .toEqual({ value: { skills: { two: '2', three: '3!' } }, conflicts: [['skills', 'three']] })
})

it('merges lists of named elements element by element, in their order, with what each side added', () => {
  const rule = (id: string, name: string) => ({ id, name, enabled: true })
  const base = [rule('a', 'A'), rule('b', 'B'), rule('c', 'C')]
  // Mine renames a, removes c, and adds d after b; theirs renames b, prepends e, and removes nothing.
  const mine = [rule('a', 'A!'), rule('b', 'B'), rule('d', 'D')]
  const theirs = [rule('e', 'E'), rule('a', 'A'), rule('b', 'B?'), rule('c', 'C')]
  expect(merged({ rules: base }, { rules: mine }, { rules: theirs }))
    .toEqual({ value: { rules: [rule('e', 'E'), rule('a', 'A!'), rule('b', 'B?'), rule('d', 'D')] }, conflicts: [] })
  // Chat rows are keyed by chatId; concurrent edits to the same row field conflict on that row only.
  const chats = (listen: string, other: string) => [{ chatId: 'oc_1', listen }, { chatId: 'oc_2', listen: other }]
  expect(merged(chats('mentions', 'mentions'), chats('all', 'mentions'), chats('none', 'all')))
    .toEqual({ value: chats('all', 'all'), conflicts: [['oc_1', 'listen']] })
})

it('merges a list without names as a whole', () => {
  expect(merged(['a'], ['a', 'b'], ['c'])).toEqual({ value: ['a', 'b'], conflicts: [[]] })
  expect(merged(['a'], ['a'], ['c'])).toEqual({ value: ['c'], conflicts: [] })
})

type Value = { mode: string; limit: number }
const saved: Value = { mode: 'off', limit: 1 }
const t = ((key: string, values?: Record<string, unknown>) => values ? `${key} ${JSON.stringify(values)}` : key) as T
const after = (actions: DraftAction<Value, undefined>[], state: DraftState<Value, undefined> = { phase: 'clean', seen: 1 }) =>
  actions.reduce(draftReducer<Value, undefined>, state)
const edit = (value: Value, from = saved, revision = 1): DraftAction<Value, undefined> => ({ type: 'edit', value, meta: undefined, saved: from,
  revision })
const arrived = (value: Value, revision: number): DraftAction<Value, undefined> =>
  ({ type: 'revisionArrived', saved: value, revision })

it('saves an edit and shows it until its revision is read back, holding edits and other revisions while the save runs', () => {
  const mine = { mode: 'auto', limit: 1 }
  const saving = after([edit(mine), { type: 'submit' }])
  expect(saving).toMatchObject({ phase: 'saving', draft: { base: saved, value: mine, revision: 1 } })
  expect(after([edit(saved), arrived(saved, 2), { type: 'discard' }], saving)).toBe(saving)
  const waiting = after([{ type: 'saved', revision: 2, restarting: true }], saving)
  expect(waiting).toMatchObject({ phase: 'awaitingRevision', sent: { value: mine, revision: 2 } })
  expect(saveBarText(t, waiting)).toEqual({ status: 'savedRestarting' })
  expect(after([arrived(mine, 2)], waiting)).toEqual({ phase: 'clean', seen: 2 })
  expect(after([edit(mine), edit(saved)])).toEqual({ phase: 'clean', seen: 1 })
  expect(after([{ type: 'edit', value: mine, meta: undefined, saved, revision: undefined }])).toMatchObject({ draft: { revision: 0 } })
})

it('merges another saved revision into a draft, tells a merge from a conflict, and keeps only the conflict across an edit', () => {
  const editing = after([edit({ mode: 'auto', limit: 1 })])
  const merged = after([arrived({ mode: 'off', limit: 5 }, 2)], editing)
  expect(merged).toMatchObject({ phase: 'editing', seen: 2, notice: { kind: 'merged', revision: 2 },
    draft: { base: { mode: 'off', limit: 5 }, value: { mode: 'auto', limit: 5 }, revision: 2 } })
  const conflict = after([arrived({ mode: 'observe', limit: 1 }, 2)], editing)
  expect(saveBarText(t, conflict)).toEqual({ error: 'saveConflict', status: '' })
  expect(after([edit({ mode: 'auto', limit: 6 }, { mode: 'off', limit: 5 }, 2)], merged).notice).toBeUndefined()
  expect(after([edit({ mode: 'auto', limit: 2 }, { mode: 'observe', limit: 1 }, 2)], conflict).notice).toMatchObject({ kind: 'conflict' })
  // Another save that made the same change leaves nothing to save.
  expect(after([arrived({ mode: 'auto', limit: 1 }, 2)], editing)).toEqual({ phase: 'clean', seen: 2 })
})

it('builds a named list from an undefined base', () => {
  expect(merged(undefined, [{ id: 'new', name: 'New' }], [{ id: 'remote', name: 'Remote' }]))
    .toEqual({ value: [{ id: 'new', name: 'New' }, { id: 'remote', name: 'Remote' }], conflicts: [] })
})

it('treats a function as a whole value and prepends a row mine added before any remote row', () => {
  const fn = () => {}
  expect(merged(fn, { mine: true }, { theirs: true })).toEqual({ value: { mine: true }, conflicts: [[]] })
  expect(merged([{ id: 'remote', name: 'Remote' }], [{ id: 'new', name: 'New' }, { id: 'remote', name: 'Remote' }], [{ id: 'remote', name: 'Remote' }]))
    .toEqual({ value: [{ id: 'new', name: 'New' }, { id: 'remote', name: 'Remote' }], conflicts: [] })
})

it('keeps a refused save as a draft with why, and words a restored draft unless its block words its drafts itself', () => {
  const saving = after([edit({ mode: 'auto', limit: 1 }), { type: 'submit' }])
  expect(saveBarText(t, after([{ type: 'refused', notice: { kind: 'rebasing' } }], saving))).toEqual({ status: 'saveRebasing' })
  const invalid = after([{ type: 'refused', notice: { kind: 'invalid', fields: ['limit', 'mode'] } }], saving)
  expect(invalid.phase).toBe('editing')
  expect(saveBarText(t, invalid)).toEqual({ error: 'invalidSettings {"fields":"limitlistSeparatormode"}', status: '' })
  const restored: DraftState<Value, undefined> = { phase: 'editing', seen: 1, notice: { kind: 'restored' },
    draft: { base: saved, value: { mode: 'auto', limit: 1 }, revision: 1 } }
  expect(saveBarText(t, restored)).toEqual({ status: 'draftRestored' })
  expect(saveBarText(t, restored, { editing: 'Version 1 · 1 change' })).toEqual({ status: 'Version 1 · 1 change' })
  expect(saveBarText(t, { phase: 'editing', notice: { kind: 'merged', revision: 7 } })).toEqual({ status: 'saveMerged' })
  expect(saveBarText(t, { phase: 'editing', notice: { kind: 'conflict', revision: 7, paths: [['mode']] } }, { conflict: revision => `Custom conflict ${revision}` }))
    .toEqual({ error: 'Custom conflict 7', status: '' })
  expect(saveBarText(t, after([arrived({ mode: 'off', limit: 3 }, 2)], restored), { merged: revision => `Prompts at ${revision}` }))
    .toEqual({ status: 'Prompts at 2' })
})

it('waits for the revision its save made, keeping the value it sent, while an older revision arrives', () => {
  const sent = { mode: 'auto', limit: 1 }
  const waiting = after([edit(sent), { type: 'submit' }, { type: 'saved', revision: 3, restarting: false }])
  const older = after([arrived(saved, 2)], waiting)
  expect(older).toMatchObject({ phase: 'awaitingRevision', seen: 2, sent: { value: sent, revision: 3 } })
  expect(saveBarText(t, older)).toEqual({ status: 'saved' })
  expect(after([arrived(sent, 3)], older)).toEqual({ phase: 'clean', seen: 3 })
})

it('keeps editing when the browser cannot store the draft, and says so', () => {
  const unstored = after([edit({ mode: 'auto', limit: 1 }), { type: 'refused', notice: { kind: 'unstored', error: 'QuotaExceededError' } }])
  expect(unstored.phase).toBe('editing')
  expect(saveBarText(t, unstored)).toEqual({ error: 'draftFailed {"error":"QuotaExceededError"}', status: '' })
  expect(after([edit({ mode: 'auto', limit: 2 })], unstored).notice).toEqual({ kind: 'unstored', error: 'QuotaExceededError' })
})

it('removes an element of a named list that mine deleted and theirs changed, and names the conflict', () => {
  const rule = (id: string, name: string) => ({ id, name })
  expect(merged([rule('a', 'A'), rule('b', 'B')], [rule('a', 'A')], [rule('a', 'A'), rule('b', 'B?')]))
    .toEqual({ value: [rule('a', 'A')], conflicts: [['b']] })
})


declare global {
  var IS_REACT_ACT_ENVIRONMENT: boolean | undefined
}
globalThis.IS_REACT_ACT_ENVIRONMENT = true

const serve = () => vi.stubGlobal('fetch', vi.fn(async () => Response.json({
  masterId: 'master', settings: SettingsSchema.parse({}), revisions: { policy: 0, eval: 0, executor: 0 }, sources: [],
  policy_version: 'v', health: {}, outgoing: [], load: { running: 0, queued: 0 },
  setup: { machine: { stateDir: '/srv/state', worktreeRoot: '/srv/state/worktrees', blockedCommands: [], promptMaxChars: 100000,
    admissionContextMaxPages: 10, admissionContextTimeoutMs: 15000 }, problem: null, feishuProblem: null, accounts: null },
})))
afterEach(() => { vi.unstubAllGlobals(); sessionStorage.clear() })

type Controls = ReturnType<typeof useDraft<Value>>
const mount = async (options: Partial<Parameters<typeof useDraft<Value>>[0]>, controls: { current?: Controls }) => {
  serve()
  function Harness() {
    const draft = useDraft<Value>({ key: 'draft', saved, revision: 1, save: async () => ({ revision: 2 }), reload: async () => {}, ...options })
    controls.current = draft
    return createElement(SaveBar, { state: draft.state, dirty: draft.dirty, busy: draft.busy, onDiscard: draft.discard,
      onSave: () => { void draft.submit() } })
  }
  const block = document.createElement('div'); document.body.append(block)
  const root = createRoot(block)
  await act(async () => { root.render(createElement(PanelProvider, { t }, createElement(Harness))) })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  return { block, root }
}

it('drops unreadable stored drafts, restores valid ones, and reports refused storage', async () => {
  sessionStorage.setItem('draft', '{bad json')
  const unreadable = { current: undefined as Controls | undefined }
  let view = await mount({ revision: undefined }, unreadable)
  expect(unreadable.current?.state).toMatchObject({ phase: 'clean' })
  act(() => { view.root.unmount() }); view.block.remove()

  sessionStorage.setItem('draft', JSON.stringify({ base: saved, value: { mode: 'auto', limit: 2 }, revision: 1 }))
  const restored = { current: undefined as Controls | undefined }
  view = await mount({ revision: undefined }, restored)
  expect(restored.current?.state).toMatchObject({ phase: 'editing', notice: { kind: 'restored' },
    draft: { value: { mode: 'auto', limit: 2 } } })
  act(() => { view.root.unmount() }); view.block.remove()

  const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('quota exceeded') })
  const unstored = { current: undefined as Controls | undefined }
  view = await mount({}, unstored)
  await act(async () => { unstored.current?.edit({ mode: 'auto', limit: 2 }) })
  for (let i = 0; i < 5; i++) await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)) })
  expect(unstored.current?.state.notice).toMatchObject({ kind: 'unstored', error: 'quota exceeded' })
  setItem.mockRestore()
  act(() => { view.root.unmount() }); view.block.remove()
})

it('submits a draft once, reports validation, conflict, and reload failures, and restarts when asked', async () => {
  const save = vi.fn(async () => ({ revision: 2, restarting: true }))
  const controls = { current: undefined as Controls | undefined }
  let view = await mount({ save }, controls)
  await act(async () => { await controls.current?.submit() })
  expect(save).not.toHaveBeenCalled()
  await act(async () => { controls.current?.edit({ mode: 'auto', limit: 2 }) })
  await act(async () => { await controls.current?.submit() })
  await act(async () => { await controls.current?.submit() })
  expect(save).toHaveBeenCalledExactlyOnceWith({ mode: 'auto', limit: 2 }, 1, saved)
  expect(controls.current?.state).toMatchObject({ phase: 'awaitingRevision', sent: { revision: 2, restarting: true } })
  act(() => { view.root.unmount() }); view.block.remove()

  const invalid = new ApiError('invalid', 400, { issues: [{ path: 'limit', message: 'too small' }] })
  const fields = { current: undefined as Controls | undefined }
  view = await mount({ save: vi.fn(async () => { throw invalid }) }, fields)
  await act(async () => { fields.current?.edit({ mode: 'auto', limit: 2 }) })
  await act(async () => { await fields.current?.submit() })
  expect(fields.current?.state.notice).toMatchObject({ kind: 'invalid', fields: ['limit'] })
  act(() => { view.root.unmount() }); view.block.remove()

  const conflict = new ApiError('conflict', 409)
  const rebasing = { current: undefined as Controls | undefined }
  view = await mount({ save: vi.fn(async () => { throw conflict }), reload: async () => {} }, rebasing)
  await act(async () => { rebasing.current?.edit({ mode: 'auto', limit: 2 }) })
  await act(async () => { await rebasing.current?.submit() })
  expect(rebasing.current?.state.notice).toEqual({ kind: 'rebasing' })
  act(() => { view.root.unmount() }); view.block.remove()

  const failedReload = new ApiError('conflict', 409)
  const failed = { current: undefined as Controls | undefined }
  view = await mount({ save: vi.fn(async () => { throw failedReload }), reload: async () => { throw new Error('reload failed') } }, failed)
  await act(async () => { failed.current?.edit({ mode: 'auto', limit: 2 }) })
  await act(async () => { await failed.current?.submit() })
  expect(failed.current?.state.notice).toMatchObject({ kind: 'failed', error: 'reload failed' })
  act(() => { view.root.unmount() }); view.block.remove()

  const refused = { current: undefined as Controls | undefined }
  view = await mount({ save: async () => { throw new Error('save failed') }, reload: async () => {} }, refused)
  await act(async () => { refused.current?.edit({ mode: 'auto', limit: 2 }) })
  await act(async () => { await refused.current?.submit() })
  expect(refused.current?.state.notice).toMatchObject({ kind: 'failed', error: 'save failed' })
  act(() => { view.root.unmount() }); view.block.remove()
})

it('returns focus to the settings block when the save bar disappears, and lets a caller override its status', async () => {
  serve()
  const discarded: number[] = []
  let shown = true
  let focusBlock: HTMLElement | undefined
  function Harness() {
    return createElement('div', { tabIndex: -1, ref: (node) => { if (node) focusBlock = node } }, createElement(SaveBar, {
      state: { phase: 'editing', draft: { base: saved, value: { mode: 'auto', limit: 2 }, revision: 1 } },
      dirty: shown, busy: false, status: 'Custom status', onDiscard: () => { discarded.push(1) }, onSave: () => {},
    }))
  }
  const block = document.createElement('div'); document.body.append(block)
  const root = createRoot(block)
  await act(async () => { root.render(createElement(PanelProvider, { t }, createElement(Harness))) })
  const save = [...block.querySelectorAll('button')].find(button => button.textContent === 'save')!
  expect(block.textContent).toContain('Custom status')
  await act(async () => { save.focus() })
  const outside = document.createElement('div'); document.body.append(outside)
  await act(async () => { save.dispatchEvent(new FocusEvent('focusout', { bubbles: true, relatedTarget: outside })) })
  await act(async () => { save.dispatchEvent(new FocusEvent('focusin', { bubbles: true })) })
  await act(async () => { save.dispatchEvent(new FocusEvent('focusout', { bubbles: true })) })
  await act(async () => { save.dispatchEvent(new FocusEvent('focusin', { bubbles: true })) })
  shown = false
  await act(async () => { root.render(createElement(PanelProvider, { t }, createElement(Harness))) })
  expect(document.activeElement).toBe(focusBlock)
  expect(discarded).toEqual([])
  outside.remove(); act(() => { root.unmount() }); block.remove()
})
