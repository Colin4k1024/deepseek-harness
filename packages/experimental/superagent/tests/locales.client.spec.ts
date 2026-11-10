import { expect, it, vi } from 'vitest'
import type { EffectKind } from '../src/app/effects.ts'
import { en, zh, type DshSuperagentKey } from '../src/client/locales.ts'
import { actorName, errorText, fullTime, localized, reasonLabel, valueLabel, when, type T } from '../src/client/text.ts'
import { REASONS, USER_ERRORS } from '../src/core/index.ts'

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort()

it('names the same placeholders in both languages', () => {
  const keys = Object.keys(zh) as DshSuperagentKey[]
  expect(keys.filter(key => placeholders(zh[key]).join() !== placeholders(en[key]).join())).toEqual([])
  expect(keys.filter(key => !zh[key] || !en[key])).toEqual([])
})

it('words a reason by its code with its values, each of a batch, and words a person or an assessor wrote as written', () => {
  const t = ((key: DshSuperagentKey, values: Record<string, string> = {}) => (zh[key] ?? key).replace(/\{(\w+)\}/g, (_match, name: string) => values[name] ?? '')) as T
  expect(reasonLabel(t, 'retried')).toBe(zh['reason.retried'])
  expect(reasonLabel(t, [{ code: 'routed', values: { ticket: 't1' } }])).toBe('已转给工单 t1')
  expect(reasonLabel(t, [{ code: 'not_a_member', triggerId: '1' }, { code: 'no_mention', triggerId: '2' }]))
    .toBe(`${zh['reason.not_a_member']}；${zh['reason.no_mention']}`)
  expect(reasonLabel(t, [{ code: 'said', values: { text: 'The button is already blue' } }])).toBe('The button is already blue')
  // A code the panel has no words for reads as the host words it.
  expect(reasonLabel(t, { code: 'some_new_filter' })).toBe('some_new_filter')
  expect(reasonLabel(t, 'The button is already blue')).toBe('The button is already blue')
  // Host errors keep only their first line, and a long first line is cut at 140 characters.
  expect(reasonLabel(t, 'CliError: boom\nsecond line')).toBe('CliError: boom')
  expect(reasonLabel(t, `Error: ${'x'.repeat(150)}\nsecond line`)).toHaveLength(141)
})

it('names every health entry of dsh-superagent itself, which the overview shows when it fails', () => {
  // Each entry by its writer: dsh-superagent, the model budget retries, every kind of the effect queue, Channel messages, the
  // Executor and the unavailable Evals of the last hour.
  const entries = { runtime: 0, models: 0, reply: 0, worker: 0, assessment: 0, launch: 0, delivery: 0, relay: 0,
    notice: 0, evals: 0 } satisfies Record<EffectKind | 'runtime' | 'models' | 'reply' | 'worker' | 'evals', 0>
  expect(Object.keys(entries).filter(name => !(`health.${name}` in zh))).toEqual([])
})

it('words every refusal the host words by code, in both languages', () => {
  expect(Object.keys(USER_ERRORS).filter(code => !zh[`userError.${code}` as DshSuperagentKey]
    || !en[`userError.${code}` as DshSuperagentKey])).toEqual([])
})

it('has words for every reason code the core records', () => {
  expect(Object.keys(REASONS).filter(code => code !== 'said' && !(`reason.${code}` in zh))).toEqual([])
})

it('words the reasons agents read in the glossary\'s words: a sender on the list, a Thread', () => {
  expect(Object.values(REASONS).filter(text => /\b(member|members|topic)\b/i.test(text))).toEqual([])
})


it('words raw values, actor names, source names, and reasons in the active language', () => {
  const t = ((key: string, values: Record<string, unknown> = {}) =>
    Object.keys(values).length ? `${key} ${JSON.stringify(values)}` : key === 'lang' ? 'en' : key === 'source.github' ? 'GitHub' : key) as T
  expect(errorText(new Error('not running'))).toBe('not running')
  expect(errorText('stopped')).toBe('stopped')
  expect(localized(t, 'plain')).toBe('plain')
  expect(localized(t, { en: 'English', zh: '中文' })).toBe('English')
  expect(localized(t, undefined)).toBe('')
  expect(actorName(t, { name: { en: 'Alice', zh: 'Alice' } })).toBe('Alice')
  expect(actorName(t, undefined)).toBe('automatedNotice')
  expect(valueLabel(t, 'source', undefined)).toBe('')
  expect(valueLabel(t, 'source', 'github')).toBe('GitHub')
  expect(valueLabel(t, 'source', 'unknown')).toBe('unknown')
  expect(reasonLabel(t, undefined)).toBe('')
  expect(reasonLabel(t, '')).toBe('')
  expect(reasonLabel(t, { code: 'said', values: { text: 'The button is already blue' } })).toBe('The button is already blue')
  expect(reasonLabel(t, { code: 'said', triggerId: 't1' })).toBe('')
})

it('words timestamps relative to now, and only names the year for other years', () => {
  vi.useFakeTimers(); vi.setSystemTime(new Date('2026-11-10T12:00:00Z'))
  expect(when(undefined, 'en')).toBe('')
  expect(when('not a date', 'en')).toBe('not a date')
  expect(when('2026-11-10T11:59:45Z', 'en')).toBe('now')
  expect(when('2026-11-10T11:58:30Z', 'en')).toBe('1 minute ago')
  expect(when('2026-11-10T08:00:00Z', 'en')).toBe('4 hours ago')
  expect(when('2026-11-01T12:00:00Z', 'en')).not.toContain('2026')
  expect(when('2027-11-01T12:00:00Z', 'en')).toContain('2027')
  expect(fullTime(undefined, 'en')).toBe('')
  expect(fullTime('not a date', 'en')).toBe('')
  expect(fullTime('2026-11-10T12:00:00Z', 'en')).toContain('2026')
  vi.useRealTimers()
})
