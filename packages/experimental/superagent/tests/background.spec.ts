/** A worker's background work in DSH: waiting for its shell jobs and subagents, and stopping them. */
import type { Context } from '@deepseek-ai/cordis'
import type { Agent } from '@deepseek-ai/dsh-agent'
import { expect, it, vi } from 'vitest'
import { collectJob, killJobs, runningJobs, stopSubagents, until, untilQuiet } from '../src/executor/background.ts'
import { jobRegistry } from './fixtures/host.ts'

/** A context with DSH's job registry and subagents, and an idle worker agent with one live subagent, `child-1`. */
function setup(waitForChildren: (agent: Agent) => Promise<boolean>, drainChildren: () => Promise<void> = async () => {}) {
  const jobs = jobRegistry()
  const agent = { id: 'worker-t1', status: 'idle', whenIdle: async () => {}, inbox: { nextTurn: [], nextStep: [] }, cancel: vi.fn() } as unknown as
    Agent & { status: string; cancel: ReturnType<typeof vi.fn> }
  const subagents = { waitForChildren: vi.fn(waitForChildren), listChildren: async () => [{ id: 'child-1' }], drainChildren: vi.fn(drainChildren) }
  const services: Record<string, unknown> = { jobs: jobs.service, subagents }
  const ctx = { get: (name: string) => services[name], agents: { get: () => agent } } as unknown as Context
  return { ctx, agent, jobs, subagents }
}

it('is not quiet when an end during the subagent check woke the worker, whose own idle looks again', async () => {
  const { ctx, agent, jobs } = setup(async () => {
    // The job settles meanwhile, and its end wakes the worker.
    jobs.end(id); (agent as { status: string }).status = 'running'
    return false
  })
  const id = jobs.run('worker-t1')
  expect(await untilQuiet(ctx, agent, new AbortController().signal)).toBe(false)
})

it('stops waiting for subagents that never settle once the host stops', async () => {
  let joining!: () => void
  const joined = new Promise<void>((resolve) => { joining = resolve })
  const { ctx, agent } = setup(() => { joining(); return new Promise(() => {}) })
  const stop = new AbortController()
  const quiet = untilQuiet(ctx, agent, stop.signal)
  // The host stops while the wait for its subagents runs.
  await joined
  stop.abort()
  expect(await quiet).toBe(false)
})

it('looks at the jobs only when one ends, not at each output of a noisy one', async () => {
  const { ctx, agent, jobs } = setup(async () => false)
  const id = jobs.run('worker-t1')
  const stop = new AbortController()
  const quiet = untilQuiet(ctx, agent, stop.signal)
  await new Promise(resolve => setTimeout(resolve, 0))
  const looked = jobs.service.list.mock.calls.length
  for (let line = 0; line < 100; line++) jobs.output(id)
  expect(jobs.service.list.mock.calls.length).toBe(looked)
  jobs.end(id)
  expect(await quiet).toBe(true)
})

it('joins a worker\'s subagents once for the looks that wait on them, as DSH cannot cancel the join', async () => {
  const { ctx, agent, subagents } = setup(() => new Promise(() => {}))
  for (let look = 0; look < 3; look++) {
    const stop = new AbortController()
    const quiet = untilQuiet(ctx, agent, stop.signal)
    stop.abort()
    expect(await quiet).toBe(false)
  }
  expect(subagents.waitForChildren).toHaveBeenCalledTimes(1)
})

it('gives up a release of subagents that does not end within 10 seconds, and cancels the turn their ends woke', async () => {
  const { ctx, agent } = setup(async () => false, () => new Promise(() => {}))
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  try {
    let stopped = false
    const stopping = stopSubagents(ctx, agent).then(() => { stopped = true })
    await vi.advanceTimersByTimeAsync(9_999)
    expect(stopped).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    await stopping
  } finally { vi.useRealTimers() }
  expect(agent.cancel).toHaveBeenCalledWith({ kind: 'user' }, { keepInbox: false })
})

it('returns false at once when the signal is already aborted', async () => {
  const stop = new AbortController()
  stop.abort()
  await expect(until(Promise.resolve('done'), stop.signal)).resolves.toBe(false)
})

it('collects the end of a shell job, and gives up collecting it once its signal aborts', async () => {
  const { ctx, jobs } = setup(async () => false)
  const running = jobs.run('worker-t1')
  const ended = collectJob(ctx, 'worker-t1', { id: running, name: running }, new AbortController().signal)
  jobs.end(running)
  await expect(ended).resolves.toEqual({ name: running, status: 'completed' })
  const stop = new AbortController()
  const aborted = collectJob(ctx, 'worker-t1', { id: running, name: running }, stop.signal)
  stop.abort()
  await expect(aborted).resolves.toBeUndefined()
})

it('collects nothing when DSH has no job registry', async () => {
  const ctx = { get: () => undefined } as unknown as Context
  await expect(collectJob(ctx, 'worker-t1', { id: 'bash-1', name: 'bash-1' }, new AbortController().signal)).resolves.toBeUndefined()
})

it('settles a job wait once even when a second end event follows the first', async () => {
  const { ctx, jobs } = setup(async () => false)
  const id = jobs.run('worker-t1')
  const quiet = untilQuiet(ctx, { id: 'worker-t1', status: 'idle', whenIdle: async () => {} } as unknown as Agent, new AbortController().signal)
  await new Promise(resolve => setTimeout(resolve, 0))
  jobs.end(id)
  jobs.end(id)
  await expect(quiet).resolves.toBe(true)
})

it('reports a kill failure while collecting a wait that also fails', async () => {
  const jobs = {
    list: () => [{ id: 'bash-1', owner: 'worker-t1', status: 'running' }],
    wait: vi.fn(async () => { throw new Error('wait failed') }),
    kill: vi.fn(() => { throw new Error('kill failed') }),
    events: { subscribe: () => () => {} },
  }
  const ctx = { get: (name: string) => name === 'jobs' ? jobs : undefined } as unknown as Context
  await expect(killJobs(ctx, 'worker-t1', 'Assignment ended')).rejects.toMatchObject({
    message: 'Killing the shell jobs of worker-t1 failed',
    errors: [new Error('kill failed')],
  })
})

it('names no running job when DSH has no job registry', () => {
  const ctx = { get: () => undefined } as unknown as Context
  expect(runningJobs(ctx, 'worker-t1')).toEqual([])
})

it('keeps collecting a job whose first wait reports it still running', async () => {
  const views = [{ status: 'running' }, { status: 'completed' }]
  const jobs = {
    list: () => [],
    wait: vi.fn(async () => views.shift()),
    kill: vi.fn(),
    events: { subscribe: () => () => {} },
  }
  const ctx = { get: (name: string) => name === 'jobs' ? jobs : undefined } as unknown as Context
  await expect(collectJob(ctx, 'worker-t1', { id: 'bash-1', name: 'bash-1' }, new AbortController().signal))
    .resolves.toEqual({ name: 'bash-1', status: 'completed' })
  expect(jobs.wait).toHaveBeenCalledTimes(2)
})
