/** Main-session assessor per-eval state and context loading. */
import { expect, it, vi } from 'vitest'
import type { Agent } from '@deepseek-ai/dsh-agent'
import { AssessmentSchema, EvalReadSchema, MasterAssessor, type AssessorHost } from '../src/app/assessment.ts'
import { mainAdmissionPolicy } from '../src/app/context.ts'
import { Engine, MemoryRepositories, type Executor, type Policy, type Trigger } from '../src/core/index.ts'
import type { Commands } from '../src/infra/commands.ts'
import { SettingsSchema } from '../src/policy.ts'
import { testDeployment } from './fixtures/deployment.ts'
import { deferred } from './fixtures/async.ts'
import { githubKind } from '../src/sources/github.ts'
import { SourceRegistry } from '../src/sources/registry.ts'

/** Assessor fixture backed by in-memory repositories and fake GitHub Issue and file reads. */
function setup(
  strength: 'explicit' | 'inferred' = 'inferred',
  bodies = Array.from({ length: 5 }, (_, i) => `comment ${i}`),
  threadContext?: AssessorHost['threadContext'],
  fileBody = 'export const value = 1\n',
  paused = false,
) {
  const comments = bodies.map((body, i) => ({
    id: i,
    html_url: `https://github.com/acme/widgets/issues/1#c${i}`,
    user: { login: 'bob' },
    created_at: '2026-09-09T00:00:00Z',
    body,
  }))
  const gate = { issue: undefined as Promise<void> | undefined }
  const commands = {
    repository: 'acme/widgets',
    deployment: testDeployment(),
    withSignal() {
      return commands
    },
    gh: vi.fn(async (args: string[]) => {
      await gate.issue
      const number = Number(/issues\/(\d+)/.exec(args[1]!)?.[1])
      if (args[1]!.startsWith('contents/')) return [{ name: 'value.ts' }]
      return args[1]!.includes('/comments')
        ? comments
        : {
          id: 100 + number,
          number,
          state: 'open',
          html_url: `https://github.com/acme/widgets/issues/${number}`,
          user: { login: 'bob' },
          created_at: '2026-09-09T00:00:00Z',
          title: 'Issue',
          body: 'body',
        }
    }),
    run: vi.fn(async () => fileBody),
    // A string first argument is a raw file path; otherwise validate the fake gh response against the schema.
    api: vi.fn(async (schema: unknown, ...rest: string[]) =>
      typeof schema === 'string'
        ? fileBody
        : (schema as { parse(value: unknown): unknown }).parse(
          await (commands as unknown as { gh(args: string[]): Promise<unknown> }).gh(['api', rest[0]!]),
        ),
    ),
  } as unknown as Commands
  const repo = new MemoryRepositories()
  const sources = new SourceRegistry(() => SettingsSchema.parse({}))
  sources.register(githubKind)
  const engines: { engine?: Engine } = {}
  const effects = { enqueue: vi.fn(), handle: vi.fn() }
  const host: AssessorHost = {
    get engine() {
      return engines.engine as Engine
    },
    commands,
    deployment: testDeployment(),
    settings: () => SettingsSchema.parse({ eval: { mode: 'auto' } }),
    threadContext: threadContext ?? (trigger => sources.threadContext(trigger)),
    effects,
    paused: () => paused,
    deliver: async () => {},
    candidateCard: async candidate => ({ eval_id: candidate.evalId }),
  }
  const assessor = new MasterAssessor(host)
  const policy: Policy = {
    version: 'v1',
    paused: false,
    evalTimeoutMs: 600_000,
    gate: () => ({ admissible: true, strength, basis: { via: 'followup' } }),
    revalidate: () => true,
    admitsInferred: rules => rules.length > 0,
  }
  const executor: Executor = { start: () => {}, deliver: () => {}, stop: () => {} }
  const scope = { scope: () => ({ listened: true as const, observed: true }) }
  const outbox = { send: () => {}, status: () => undefined, withdraw: () => {} }
  const engine = new Engine({ repo, policy: () => policy, scope, assessor, executor, outbox })
  engines.engine = engine
  const tools: Record<string, (args: unknown, signal: AbortSignal) => Promise<unknown>> = {}
  tools.orchestrator_eval_read = (args, signal) => assessor.read({} as Agent, EvalReadSchema.parse(args), signal)
  tools.orchestrator_eval_decide = (args, signal) => {
    const { trigger_id, ...result } = args as { trigger_id: string }
    return assessor.decide(trigger_id, AssessmentSchema.parse(result), signal)
  }
  // Per-Eval state retained by the assessor and released when the Eval ends.
  const kept = assessor as unknown as { evals: Map<string, unknown> }
  const size = () => kept.evals.size
  return { repo, engine, assessor, tools, size, gate, effects }
}
const issue = (n: number): Trigger => ({ id: `issue-${n}`, source: 'github', threads: [{ source: 'github', id: String(n) }], actor: { source: 'github', id: 'bob' }, human: true,
  at: new Date(Date.UTC(2026, 8, 9, 0, 0, n)).toISOString(), text: `issue ${n}`, url: `https://github.com/acme/widgets/issues/${n}`, meta: { number: n } })
const signal = new AbortController().signal

it('keeps nothing for an Eval once it is decided, as for one that timed out', async () => {
  const s = setup()
  for (let n = 1; n <= 3; n++) {
    s.engine.receive(issue(n)); s.engine.evaluateDue()
    await s.assessor.context(`issue-${n}`, signal)
    await s.tools.orchestrator_eval_read!({ trigger_id: `issue-${n}`, resource: 'file', path: 'src/value.ts', offset: 0 }, signal)
    expect(s.size()).toBeGreaterThan(0)
    s.engine.decide(`issue-${n}`, { kind: 'ignore', reason: 'not a request' })
    expect(s.size()).toBe(0)
  }
  s.engine.receive(issue(9)); s.engine.evaluateDue()
  await s.assessor.context('issue-9', signal)
  const thread = s.repo.threads.get('github:9')!
  s.repo.threads.save({ ...thread, expired: 2, evaluating: { ...thread.evaluating!, delivered: '2000-01-01T00:00:00.000Z', deadline: '2000-01-01T00:00:00.000Z' } })
  s.engine.evaluateDue()
  expect(s.size()).toBe(0)
})

it('keeps nothing from a context read that finishes after its Eval was decided', async () => {
  const s = setup()
  const hold = deferred()
  s.gate.issue = hold.promise
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const details = s.assessor.context('issue-1', signal)
  s.engine.decide('issue-1', { kind: 'ignore', reason: 'not a request' })
  hold.resolve()
  await expect(details).rejects.toThrow('This assessment has finished')
  expect(s.size()).toBe(0)
})

it('reuses the initial context load and works without a caller signal', async () => {
  const s = setup()
  const hold = deferred()
  s.gate.issue = hold.promise
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const first = s.assessor.context('issue-1')
  await Promise.resolve()
  const second = s.assessor.context('issue-1', signal)
  hold.resolve()
  expect(await first).toEqual(await second)
})

it('drops a failed initial context read whose evaluation ended while it was loading', async () => {
  const s = setup()
  const hold = deferred()
  s.gate.issue = hold.promise
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const details = s.assessor.context('issue-1', signal)
  s.engine.decide('issue-1', { kind: 'ignore', reason: 'not a request' })
  hold.reject(new Error('Feishu unavailable'))
  await expect(details).rejects.toThrow('This assessment has finished')
  expect(s.size()).toBe(0)
})

it('reads the context of a Trigger through its Source, and none for a Source that gives none', async () => {
  const s = setup()
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const context = await s.assessor.context('issue-1', signal)
  expect(context.thread).toMatchObject({ complete: true, loaded_messages: 6, source_found: true })
  const page = await s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'thread', offset: 0 }, signal) as { text: string }
  expect(JSON.parse(page.text)).toMatchObject({ next_read: null,
    messages: expect.arrayContaining([expect.objectContaining({ text: 'comment 0' })]) as unknown })
  const other: Trigger = { ...issue(2), id: 'other-2', source: 'probe', threads: [{ source: 'probe', id: '2' }], actor: { source: 'probe',
    id: 'bob' } }
  s.engine.receive(other); s.engine.evaluateDue()
  expect((await s.assessor.context('other-2', signal)).thread).toMatchObject({ error: expect.stringContaining('gives no Thread context') as unknown })
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'other-2', resource: 'thread', offset: 0 },
    signal)).rejects.toThrow('gives no Thread context')
  // The Trigger's own text is still readable when its Source provides no Thread context.
  expect(await s.tools.orchestrator_eval_read!({ trigger_id: 'other-2', resource: 'message', offset: 0 },
    signal)).toMatchObject({ text: 'issue 2' })
})
it('routes an explicit candidate instead of assessing it: each eval tool names the routing tools', async () => {
  const s = setup('explicit')
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const routed = 'orchestrator_route with its trigger_id and kind accept, attach, stop or ignore'
  await expect(s.assessor.context('issue-1', signal)).rejects.toThrow(routed)
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'message', offset: 0 }, signal)).rejects.toThrow(routed)
  const direction = { implementation_requested: false, target_basis: 'undetermined', requires_design_decision: false, evidence: 'chatter' }
  const decision = { trigger_id: 'issue-1', decision: 'ignore', rule_assessments: [], direction, title: 't', scope: 's', reason: 'r' }
  await expect(s.tools.orchestrator_eval_decide!(decision, signal)).rejects.toThrow(routed)
})

it('asks the assessment only for decisions its tool accepts, which no longer include reopen or stop', () => {
  expect(mainAdmissionPolicy).not.toMatch(/reopen|decision=stop/)
  const direction = { implementation_requested: true, target_basis: 'specified', requires_design_decision: false, evidence: 'e' }
  const result = (decision: string, extra = {}) =>
    AssessmentSchema.safeParse({ decision, rule_assessments: [], direction, title: 't', scope: 's', reason: 'r', ...extra }).success
  expect([result('ignore'), result('unavailable'), result('reopen'), result('stop')]).toEqual([true, true, false, false])
  expect(result('ignore', { ticket_id: 't' })).toBe(false)
})

it('loads a window without the assessment instructions: the trigger once, cut messages with their length, and stubs for the rest',
  async () => {
    const bodies = Array.from({ length: 30 }, (_, i) => `comment ${i}: ${'the label reads oddly on the settings page. '.repeat(i % 4 === 0
      ? 40 : 3)}`)
    const s = setup('inferred', bodies)
    const trigger = { ...issue(1), id: 'c15', text: bodies[15]!, url: 'https://github.com/acme/widgets/issues/1#c15', meta: { number: 1,
      kind: 'comment' } }
    s.engine.receive(trigger); s.engine.evaluateDue()
    const result = await s.assessor.context('c15', signal) as Record<string, unknown> & { thread: { messages: Record<string, unknown>[] } }
    expect(result).not.toHaveProperty('instructions')
    expect(result.text).toBe(bodies[15])
    const messages = new Map(result.thread.messages.map(message => [message.id as string, message]))
    const at = (i: number) => messages.get(`https://github.com/acme/widgets/issues/1#c${i}`)!
    // The trigger's own text is given once at top level; its two nearest neighbours keep more text than the rest of the window.
    expect(at(15)).toEqual({ id: expect.any(String) as unknown, from: 'bob', at: expect.any(String) as unknown, trigger: true })
    expect(at(16)).toMatchObject({ chars: bodies[16]!.length, text: `${bodies[16]!.slice(0, 1200)}…` })
    expect(at(12)).toMatchObject({ chars: bodies[12]!.length, text: `${bodies[12]!.slice(0, 400)}…` })
    expect(at(14)).toMatchObject({ text: bodies[14] })
    expect(at(14)).not.toHaveProperty('chars')
    // Messages outside the window are stubs that orchestrator_eval_read can read in full.
    expect(at(8)).toEqual({ id: expect.any(String) as unknown, from: 'bob', preview: bodies[8]!.slice(0, 50) })
    const whole = await s.tools.orchestrator_eval_read!({ trigger_id: 'c15', resource: 'message', message_id: at(8).id }, signal)
    expect(JSON.stringify(whole)).toContain(bodies[8]!.slice(-40))
    expect(JSON.stringify(result).length).toBeLessThan(9000)
  })

it('lets the main Session read on from the cursor its loaded window ends with, and points a cut batch message at its whole text', async () => {
  // Every page returns a next cursor so the bounded window ends with a continuation cursor.
  const page = (cursor?: string) => {
    const n = Number(cursor ?? 0)
    const message = { id: `m${n}`, from: 'bob', at: new Date(Date.UTC(2026, 8, 9, 0, 0, n)).toISOString(), text: `message ${n}` }
    return { messages: [message], next: String(n + 1), body: { messages: [message] } }
  }
  const context = { messageId: 'm0', read: async (_commands: Commands, _part: string, cursor?: string) => page(cursor), images: () => [],
    image: async () => { throw new Error('No images') } }
  const s = setup('inferred', undefined, () => context)
  s.engine.receive(issue(1))
  const long = { ...issue(2), id: 'long', threads: issue(1).threads, text: 'a long follow-up. '.repeat(40) }
  s.engine.receive(long); s.engine.evaluateDue()
  const result = await s.assessor.context('long', signal) as { thread: { next_cursor?: string }; batch: Record<string, unknown>[] }
  expect(result.thread.next_cursor).toBe('10')
  const more = await s.tools.orchestrator_eval_read!({ trigger_id: 'long', resource: 'thread', cursor: '10' }, signal)
  expect(JSON.stringify(more)).toContain('message 10')
  const shorter = { trigger_id: 'issue-1' }
  expect(result.batch).toEqual([expect.objectContaining(shorter)])
  const s2 = setup('inferred', undefined, () => context)
  s2.engine.receive(long); s2.engine.receive({ ...issue(3), threads: issue(1).threads }); s2.engine.evaluateDue()
  const other = await s2.assessor.context('issue-3', signal) as { batch: Record<string, unknown>[] }
  expect(other.batch).toEqual([expect.objectContaining({ trigger_id: 'long', chars: long.text.length, read: 'trigger/long' })])
})


it('rejects assessment and read arguments the tools cannot accept', () => {
  const direction = { implementation_requested: true, target_basis: 'specified', requires_design_decision: false, evidence: 'e' }
  const assessment = { decision: 'ignore', direction, title: 't', scope: 's', reason: 'r' }
  expect(AssessmentSchema.safeParse({ ...assessment, rule_assessments: [
    { rule_id: 'copy', reason: 'one', matched: true }, { rule_id: 'copy', reason: 'two', matched: false },
  ] }).success).toBe(false)
  expect(EvalReadSchema.safeParse({ trigger_id: 't', resource: 'file' }).success).toBe(false)
  expect(EvalReadSchema.safeParse({ trigger_id: 't', resource: 'message', path: 'x' }).success).toBe(false)
  expect(EvalReadSchema.safeParse({ trigger_id: 't', resource: 'message', cursor: 'c' }).success).toBe(false)
  expect(EvalReadSchema.safeParse({ trigger_id: 't', resource: 'thread', message_id: 'm' }).success).toBe(false)
  expect(EvalReadSchema.safeParse({ trigger_id: 't', resource: 'image' }).success).toBe(false)
})

it('ignores an assessment effect whose eval no longer owns the trigger, and pauses new deliveries while processing is paused', async () => {
  const s = setup('inferred', undefined, undefined, undefined, true)
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  await s.assessor.context('issue-1', signal)
  const [kind, handler, options] = s.effects.handle.mock.calls[0] as [string,
    (value: { triggerId: string }, evalId: string) => Promise<void>, { limit: () => number }]
  expect(kind).toBe('assessment')
  expect(options.limit()).toBe(0)
  await handler({ triggerId: 'issue-1' }, 'another-eval')
  expect(s.size()).toBe(1)
})

it('refuses reads for a trigger with no active evaluation', async () => {
  const s = setup()
  await expect(s.assessor.context('not-running', signal)).rejects.toThrow('No assessment is in progress')
})

it('exposes and reads image references from thread context, and refuses stale or unknown reads', async () => {
  let context: AssessorHost['threadContext'] = {
    messageId: 'm0',
    read: async () => ({
      messages: [
        { id: 'm0', from: 'bob', at: '2026-09-09T00:00:00Z', text: 'look at image-key', bot: true },
        { id: 'm1', from: 'bob', at: '2026-09-09T00:00:01Z', text: 'deleted', deleted: true },
        { id: 'm2', from: 'bob', at: '2026-09-09T00:00:02Z', text: 'also image-key' },
      ],
      body: {},
    }),
    images: (text: string) => text.includes('image-key') ? ['image-key'] : [],
    image: vi.fn(async (...args: unknown[]) => ({ delivered: args.length })),
  }
  const s = setup('inferred', undefined, () => context)
  s.engine.receive({ ...issue(1), url: undefined }); s.engine.evaluateDue()
  const initial = await s.assessor.context('issue-1', signal) as { images?: string[]; source: { url?: string }; thread: { messages: unknown[] } }
  expect(initial.source).not.toHaveProperty('url')
  expect(initial.thread.messages).toEqual(expect.arrayContaining([
    expect.objectContaining({ id: 'm0', bot: true }),
    expect.objectContaining({ id: 'm1', deleted: true }),
  ]))
  expect(await s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'image', image_id: 'm0/image-key' }, signal))
    .toEqual({ delivered: 6 })
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'image', image_id: 'm0/image-key', offset: 1 }, signal))
    .rejects.toThrow('Images are not paginated')
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'image', image_id: 'missing/image-key' }, signal))
    .rejects.toThrow('Unknown image reference')
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'message', message_id: 'missing' }, signal))
    .rejects.toThrow('Unknown message reference')
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'thread', cursor: 'unknown' }, signal))
    .rejects.toThrow('Unknown thread cursor')
  context = undefined
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'image', image_id: 'm0/image-key' }, signal))
    .rejects.toThrow('gives no Thread context')
})

it('serves repository pages from offset zero and later offsets from the cache', async () => {
  const s = setup('inferred', undefined, undefined, 'x'.repeat(5000))
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  await s.assessor.context('issue-1', signal)
  await expect(s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'file', path: 'src/value.ts', offset: 1 }, signal))
    .rejects.toThrow('Read this resource at offset 0 first')
  const first = await s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'file', path: 'src/value.ts', offset: 0 }, signal) as { text: string; next_offset: number | null }
  const next = await s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'file', path: 'src/value.ts', offset: first.next_offset ?? 0 }, signal) as { offset: number }
  expect(first.next_offset).toBe(4000)
  expect(next.offset).toBe(4000)
  expect(await s.tools.orchestrator_eval_read!({ trigger_id: 'issue-1', resource: 'directory', path: 'src', offset: 0 }, signal))
    .toMatchObject({ resource: 'directory' })
})

it('records a failed initial thread read instead of losing the evaluation', async () => {
  const context: AssessorHost['threadContext'] = {
    messageId: 'm0',
    read: async () => { throw new Error('Feishu unavailable') },
    images: () => [],
    image: async () => { throw new Error('No images') },
  }
  const s = setup('inferred', undefined, () => context)
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  const result = await s.assessor.context('issue-1', signal) as { thread: { error: string } }
  expect(result.thread.error).toContain('Thread context unavailable: Error: Feishu unavailable')
})

it('records an accepted assessment with its acknowledgement and ticket', async () => {
  const s = setup()
  s.engine.receive(issue(1)); s.engine.evaluateDue()
  await s.assessor.context('issue-1', signal)
  const decision = {
    decision: 'accept',
    rule_assessments: [
      { rule_id: 'copy', reason: 'A person asked for a wording fix.', matched: true },
      { rule_id: 'visual', reason: 'No visual evidence.', matched: false },
    ],
    direction: { implementation_requested: true, target_basis: 'specified', requires_design_decision: false, evidence: 'The message asks to fix the label.' },
    title: 'Fix label',
    worktree_name: 'fix-label',
    scope: 'Fix the settings label.',
    intent_evidence: '“修一下标签”',
    acknowledgement: '收到。',
    reason: 'A person asked for a copy fix.',
  }
  const result = await s.tools.orchestrator_eval_decide!({ trigger_id: 'issue-1', ...decision }, signal) as { state: string; ticket_id?: string }
  expect(result).toMatchObject({ state: 'accept', ticket_id: expect.any(String) as unknown })
  expect(s.repo.tickets.get(result.ticket_id ?? '')?.mandate.strength).toBe('inferred')
})
