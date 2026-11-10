import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { describe, expect, it, vi } from 'vitest'
import { spawn } from 'node:child_process'
import { Commands } from '../src/infra/commands.ts'
import { runFeishu } from '../src/sources/feishu-loops.ts'
import type { FeishuConfig } from '../src/sources/feishu.ts'
import { testDeployment } from './fixtures/deployment.ts'

vi.mock('node:child_process', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:child_process')>()
  return { ...actual, spawn: vi.fn() }
})

class FakeChild extends EventEmitter {
  readonly stdout = new PassThrough()
  readonly stderr = new PassThrough()
  readonly stdin = { end: vi.fn() }
  readonly kill = vi.fn(() => { this.emit('error', new Error('killed while stopping')) })
}

const spawnMock = vi.mocked(spawn)

describe('Feishu watch loop', () => {
  it('does not report an error when stopping makes the bot child fail', async () => {
    const deployment = testDeployment()
    const controller = new AbortController()
    const commands = new Commands(() => deployment, {}, controller.signal)
    const report = vi.fn()
    let configCalls = 0
    const ctx = {
      ready: Promise.resolve(),
      configured: () => true,
      deployment,
      commands,
      config: vi.fn((): FeishuConfig | undefined => (configCalls++ === 0 ? ({} as FeishuConfig) : undefined)),
      receive: vi.fn(() => 'stored' as const),
      atomic: vi.fn(<T>(work: () => T): T => work()),
      state: {
        get: vi.fn(() => undefined),
        set: vi.fn(() => {}),
        keys: vi.fn(() => []),
      },
      threads: { get: vi.fn(() => undefined), subscribed: vi.fn(() => []), describe: vi.fn(), patch: vi.fn() },
      tickets: { get: vi.fn(() => undefined), open: vi.fn(() => []), bindThread: vi.fn(), threadClosed: vi.fn() },
      triggers: { get: vi.fn(() => undefined), latest: vi.fn(() => undefined) },
      notify: vi.fn(),
      report,
    } as never
    const child = new FakeChild()
    spawnMock.mockImplementationOnce(() => child as never)

    const running = runFeishu(ctx, controller.signal)
    await vi.waitFor(() => { expect(spawnMock).toHaveBeenCalledOnce() })
    child.stderr.write('[event] ready\n')
    await vi.waitFor(() => { expect(report).toHaveBeenCalledWith('feishu', expect.objectContaining({ state: 'ready' })) })

    controller.abort()
    await running
    expect(report).not.toHaveBeenCalledWith('feishu', expect.objectContaining({ state: 'error' }))
  })
})
