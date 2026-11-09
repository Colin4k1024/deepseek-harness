/** Executor and Source environment-variable projection contracts. */
import { describe, expect, it } from 'vitest'
import { z } from 'zod'
import { environmentOf, ticketEnvironment } from '../src/app/execution-environment.ts'
import type { Ticket, Trigger } from '../src/core/index.ts'
import { SettingsSchema } from '../src/policy.ts'
import { SourceRegistry } from '../src/sources/registry.ts'
import type { SourceKind } from '../src/sources/kind.ts'
import { testDeployment } from './fixtures/deployment.ts'

const probe: SourceKind<{ interval: number }> = {
  kind: 'probe',
  title: { zh: '探针', en: 'Probe' },
  description: { zh: '', en: '' },
  health: [],
  schema: z.object({ interval: z.number().int().default(1) }).strict(),
  fields: [],
  environment: { variables: [{ key: 'DSH_PROBE_PRESENT', description: { zh: '', en: '' } }], values: () => ({}) },
}

describe('execution environment', () => {
  it('keeps declared Source variables present and fills an absent ticket execution with empty values', () => {
    const settings = SettingsSchema.parse({ sources: { probe: { enabled: true, config: {} } } })
    const sources = new SourceRegistry(() => settings)
    sources.register(probe)
    const deployment = testDeployment()
    expect(environmentOf({ deployment, sources, settings })).toContainEqual({
      key: 'DSH_PROBE_PRESENT', value: '', description: { zh: '', en: '' }, owner: 'probe',
    })
    const ticket: Ticket = {
      id: 'ticket-1', title: 'Probe', decisionId: 'decision-1',
      mandate: { strength: 'explicit', policyVersion: 'v1', basis: { via: 'listed', sender: 'Alice' },
        actor: { source: 'github', id: 'alice' }, scope: 'small' },
      channels: [], assignments: [], inputs: [], undelivered: [], createdAt: '', updatedAt: '',
      origin: { triggerId: 'trigger-1', triggerIds: ['trigger-1'] },
    }
    const trigger: Trigger = { id: 'trigger-1', source: 'github', threads: [{ source: 'github', id: '42' }],
      actor: { source: 'github', id: 'alice' }, human: true, at: '', text: 'Fix it', meta: {} }
    const variablesOf = (input: Parameters<typeof ticketEnvironment>[0]) =>
      Object.fromEntries(ticketEnvironment(input).map(item => [item.key, item.value]))
    const withTrigger = variablesOf({ deployment, sources, settings, ticket, trigger })
    const withoutTrigger = variablesOf({ deployment, sources, settings, ticket })
    expect(withTrigger).toMatchObject(withoutTrigger)
    expect(withoutTrigger).toMatchObject({ DSH_SUPERAGENT_NAME: '', DSH_SUPERAGENT_DIR: '', DSH_SUPERAGENT_TICKET: 'ticket-1' })
  })
})
