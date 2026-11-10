import { expect, it, vi } from 'vitest'
import { Config, type Machine } from '../src/deployment.ts'
import { apply } from '../src/index.ts'

// Stub Runtime to capture the resolved Machine settings and restart callback.
const runtimes = vi.hoisted(() => [] as { machine: Machine; restart: () => void; stop: ReturnType<typeof vi.fn> }[])
const configured = vi.hoisted(() => ({ value: true }))
const setupProblemState = vi.hoisted(() => ({ value: { detail: 'no state directory is configured' } as { detail: string } | undefined }))
vi.mock('../src/app/runtime.ts', () => ({
  Runtime: class {
    ready = Promise.resolve()
    constructor(
      _ctx: unknown,
      readonly machine: Machine,
      readonly restart: () => void,
    ) {
      runtimes.push(this)
    }
    configured() {
      return configured.value
    }
    setupProblem = setupProblemState.value
    stop = vi.fn(async () => {})
  },
}))
/** Simulate the Cordis loader updating a volatile configuration reference in place. */
const commit = (reference: unknown, value: unknown) => {
  ;(reference as Record<symbol, (value: unknown) => void>)[Symbol.for('cosmokit.volatile.write')]!(value)
}

function context() {
  const handlers = new Map<string, () => void>()
  const effects: Array<() => unknown> = []
  const configure = vi.fn(() => () => {})
  const describe = vi.fn(() => [])
  const ctx = {
    on: vi.fn((name: string, handler: () => void) => {
      handlers.set(name, handler)
    }),
    inject: vi.fn((_services: string[], callback: (child: unknown) => void) => {
      callback({ effect: (effect: () => unknown) => effect(), settings: { configure } })
    }),
    effect: vi.fn((effect: () => unknown) => effects.push(effect())),
    fiber: { restart: vi.fn(async () => {}) },
    get: vi.fn((name: string) => (name === 'settings' ? { describe } : undefined)),
    logger: { info: vi.fn(), error: vi.fn() },
  }
  return { ctx, handlers, configure, describe, effects }
}

it('reads a limit DSH commits a new value of without a restart, and asks DSH to generate no settings page', async () => {
  const { ctx, handlers, configure } = context()
  const config = Config({ stateDir: '/srv/state', promptMaxChars: 5000 })
  await apply(ctx as never, config)
  expect(configure).toHaveBeenCalledExactlyOnceWith({ auto: false }, ctx.fiber)
  const { machine } = runtimes.at(-1)!
  expect(machine.promptMaxChars.get()).toBe(5000)
  commit(config.promptMaxChars, 8000)
  expect(machine.promptMaxChars.get()).toBe(8000)
  expect(handlers.has('loader/volatile-update')).toBe(false)
  expect(ctx.fiber.restart).not.toHaveBeenCalled()
})

it('asks DSH to read its settings entry again once a restart it started finished', async () => {
  const { ctx, describe } = context()
  let finish = () => {}
  ctx.fiber.restart.mockImplementation(() => new Promise<void>((resolve) => { finish = resolve }))
  await apply(ctx as never, Config({ stateDir: '/srv/state' }))
  // Saving a Source account change restarts dsh-superagent, which re-runs setup and refreshes the settings descriptor.
  runtimes.at(-1)!.restart()
  await Promise.resolve()
  expect(describe).not.toHaveBeenCalled()
  finish()
  await vi.waitFor(() => { expect(describe).toHaveBeenCalledTimes(1) })
})

it('stops the Runtime when the plugin is disposed', async () => {
  const { ctx, effects } = context()
  await apply(ctx as never, Config({ stateDir: '/srv/state' }))
  const stop = effects.find(effect => typeof effect === 'function')
  if (stop === undefined) throw new Error('plugin registered no disposal effect')
  await stop()
  expect(runtimes.at(-1)!.stop).toHaveBeenCalledOnce()
})

it('reports a failed restart and a Runtime waiting for setup', async () => {
  const { ctx, describe } = context()
  configured.value = false
  ctx.fiber.restart.mockRejectedValueOnce(new Error('fiber stopped'))
  await apply(ctx as never, Config({ stateDir: '/srv/state' }))
  expect(ctx.logger.info).toHaveBeenCalledWith(expect.stringContaining('waiting for setup: no state directory is configured'))
  runtimes.at(-1)!.restart()
  await vi.waitFor(() => { expect(ctx.logger.error).toHaveBeenCalledWith('dsh-superagent restart failed: %s', 'Error: fiber stopped') })
  expect(describe).not.toHaveBeenCalled()
})

it('reports an empty setup detail when the Runtime has no setup problem', async () => {
  const { ctx } = context()
  configured.value = false
  setupProblemState.value = undefined
  try {
    await apply(ctx as never, Config({ stateDir: '/srv/state' }))
    expect(ctx.logger.info).toHaveBeenCalledWith(expect.stringContaining('waiting for setup: '))
  } finally {
    setupProblemState.value = { detail: 'no state directory is configured' }
  }
})

it('refuses keys of the previous configuration before registering anything', async () => {
  const { ctx } = context()
  await expect(apply(ctx as never, Config({ stateDir: '/srv/state', github: { cli: 'gh' } }))).rejects.toThrow('github (now on the GitHub Source)')
  expect(ctx.on).not.toHaveBeenCalled()
  expect(ctx.inject).not.toHaveBeenCalled()
})
