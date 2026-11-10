/** Concurrency, retry, and validation tests for settings saves, pause toggling, and policy synchronization. */
import { spawn } from 'node:child_process'
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, expect, it, vi } from 'vitest'
import { KV } from '../src/kv.ts'
import { UserError } from '../src/core/index.ts'
import { Commands } from '../src/infra/commands.ts'
import { Setup } from '../src/app/setup.ts'
import { SettingsConflict } from '../src/app/settings.ts'
import { startRuntime, type Host } from './fixtures/host.ts'
import { testMachine } from './fixtures/deployment.ts'
import { SettingsSchema, type SettingsBlock } from '../src/policy.ts'
import type { SubprocessRuntime } from '@deepseek-ai/dsh-subprocess'
import { writeEnvironment } from '../src/infra/environment.ts'

vi.mock('../src/infra/environment.ts', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../src/infra/environment.ts')>()
  return { ...actual, writeEnvironment: vi.fn(actual.writeEnvironment) }
})

let h: Host | undefined
afterEach(async () => { await h?.stop(); h = undefined })
function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((settle) => { resolve = settle })
  return { promise, resolve }
}
const model = { provider: 'worker-provider', model: 'worker-model' }

it('refuses a save of a block that another save changed during its model or account check, and saves nothing of it', async () => {
  h = await startRuntime()
  const { settings, store } = h.runtime
  const llm = (h.ctx as unknown as { llm: { resolveCallConfig: ReturnType<typeof vi.fn<() => Promise<unknown>>> } }).llm
  const check = deferred<unknown>()
  llm.resolveCallConfig.mockImplementationOnce(() => check.promise)
  const executor = store.settings().executor
  const revision = store.blockRevision('executor')
  const slow = settings.save('executor', { ...executor, model, maxWorkers: 5 }, revision, 'panel')
  await settings.save('executor', { ...executor, maxWorkers: 2 }, revision, 'panel')
  check.resolve(model)
  await expect(slow).rejects.toThrow(SettingsConflict)
  expect(store.settings().executor).toMatchObject({ maxWorkers: 2, model: executor.model })
  // An account check that a save of the same Source outruns is refused the same way.
  const accounts = deferred<undefined>()
  vi.spyOn(Setup.prototype, 'checkAccounts').mockImplementationOnce(() => accounts.promise)
  const github = settings.block('source:github')
  const value = github.value as { enabled: boolean; config: Record<string, unknown> }
  const moved = settings.save('source:github', { ...value, config: { ...value.config, cli: '/opt/bin/gh-other' } }, github.revision,
    'panel')
  await settings.save('source:github', { ...value, config: { ...value.config, pollSeconds: 90 } }, github.revision, 'panel')
  accounts.resolve(undefined)
  await expect(moved).rejects.toThrow(SettingsConflict)
  expect(store.settings().sources.github!.config).toMatchObject({ cli: value.config.cli, pollSeconds: 90 })
})

it('refuses a save whose revision is already stale before any check runs', async () => {
  h = await startRuntime()
  const { settings, store } = h.runtime
  const executor = store.settings().executor
  await expect(settings.save('executor', { ...executor, maxWorkers: 5 }, store.blockRevision('executor') - 1, 'panel'))
    .rejects.toThrow(SettingsConflict)
  expect(store.settings().executor).toEqual(executor)
})

it('refuses a Policy sync that a Policy save outran, reports it on sync:feishu, and keeps the save', async () => {
  h = await startRuntime()
  const { settings, store } = h.runtime
  const members = deferred<unknown>()
  vi.spyOn(Commands.prototype, 'lark').mockImplementationOnce(() => members.promise)
  const syncing = settings.sync('feishu')
  const policy = store.settings().policy
  const rules = [{ id: 'docs', name: 'Docs', enabled: true, instruction: 'Fix docs' }]
  await settings.save('policy', { ...policy, rules }, store.blockRevision('policy'), 'panel')
  members.resolve({ users: [{ member_id: 'ou_alice', name: 'Alice' }, { member_id: 'ou_new', name: 'New' }], has_more: false })
  await expect(syncing).rejects.toThrow(SettingsConflict)
  expect(h.runtime.health['sync:feishu']).toMatchObject({ state: 'error',
    detail: expect.stringContaining('Policy changed during synchronization') as unknown })
  expect(store.settings().policy).toMatchObject({ rules, sources: { feishu: { allowed: [{ id: 'ou_alice', name: 'Alice' }] } } })
})

it('syncs a Policy part again after a failed sync, a minute later and then after a wait that doubles, until it succeeds', async () => {
  h = await startRuntime()
  const { settings } = h.runtime
  // The syncs the Runtime runs as it starts finish first.
  await vi.waitFor(() =>{  expect(h!.runtime.health['sync:feishu']).toMatchObject({ state: 'ready' }) })
  await h.runtime.idle()
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  try {
    const lark = vi.spyOn(Commands.prototype, 'lark')
    lark.mockRejectedValueOnce(new Error('network down')).mockRejectedValueOnce(new Error('network down'))
    await expect(settings.sync('feishu')).rejects.toThrow('network down')
    expect(h.runtime.health['sync:feishu']).toMatchObject({ state: 'error', detail: expect.stringContaining('network down') as unknown })
    const calls = () => lark.mock.calls.filter(([args]) => args.includes('+chat-members-list')).length
    const before = calls()
    await vi.advanceTimersByTimeAsync(60_000)
    // The first retry fails too, so the next waits two minutes.
    expect(calls()).toBe(before + 1)
    lark.mockResolvedValueOnce({ users: [{ member_id: 'ou_alice', name: 'Alice' }], has_more: false })
    await vi.advanceTimersByTimeAsync(60_000)
    expect(calls()).toBe(before + 1)
    await vi.advanceTimersByTimeAsync(60_000)
    await vi.waitFor(() =>{  expect(h!.runtime.health['sync:feishu']).toMatchObject({ state: 'ready' }) })
    expect(calls()).toBe(before + 2)
    // A success ends the retries.
    await vi.advanceTimersByTimeAsync(3_600_000)
    expect(calls()).toBe(before + 2)
    const revision = h.runtime.store.blockRevision('policy')
    lark.mockResolvedValueOnce({ users: [{ member_id: 'ou_alice', name: 'Alice' }], has_more: false })
    await expect(h.runtime.settings.sync('feishu')).resolves.toMatchObject({ allowed: [{ id: 'ou_alice', name: 'Alice' }] })
    expect(h.runtime.store.blockRevision('policy')).toBe(revision)
  } finally { vi.useRealTimers() }
})
it('records nothing for a pause that changes nothing, refuses a sync while setup waits, and names an unknown block', async () => {
  h = await startRuntime()
  const { settings, store } = h.runtime
  const revision = store.blockRevision('pause')
  const inject = h.agents.get(h.runtime.masterId)!.inject
  inject.mockClear()
  expect(await settings.pause(false, 'panel')).toEqual({ paused: false })
  await h.runtime.idle()
  expect([store.blockRevision('pause'), inject.mock.calls.length, h.runtime.effects.pending('notice')]).toEqual([revision, 0, false])
  vi.spyOn(Setup.prototype, 'configured').mockReturnValue(false)
  await expect(settings.sync('feishu')).rejects.toThrow(new UserError('nothingToSync', { source: 'feishu' }))
  vi.spyOn(Setup.prototype, 'configured').mockRestore()
  expect(() => settings.block('source:x')).toThrow(new UserError('unknownBlock', { block: 'source:x' }))
  await expect(settings.save('unknown' as SettingsBlock, {}, 0, 'panel'))
    .rejects.toThrow(new UserError('unknownBlock', { block: 'unknown' }))
})

it('exposes the current settings, unchanged syncs, label refreshes, and validation failures', async () => {
  h = await startRuntime()
  const { settings, store } = h.runtime
  expect(settings.current()).toEqual(store.settings())
  store.delete(KV.settings.revision)
  expect(settings.view()).toMatchObject({ revision: 0 })
  expect(settings.view().policy.read).toBe('settings/policy')
  const llm = (h.ctx as unknown as { llm: { resolveCallConfig: ReturnType<typeof vi.fn<() => Promise<unknown>>> } }).llm
  llm.resolveCallConfig.mockRejectedValueOnce('provider unavailable')
  const executor = store.settings().executor
  await expect(settings.save('executor', { ...executor, model }, store.blockRevision('executor'), 'panel'))
    .rejects.toThrow(new UserError('modelUnavailable', { block: 'executor', error: 'provider unavailable' }))
  await vi.waitFor(() =>{  expect(h!.runtime.health['sync:feishu']).toMatchObject({ state: 'ready' }) })
  expect(await settings.sync('feishu')).toEqual(store.settings().policy.sources.feishu)
  const part = store.settings().policy.sources.feishu!
  const sync = h.runtime.sources.kind('feishu')!.policy!.sync!
  const run = vi.spyOn(sync, 'run').mockResolvedValueOnce(part)
  await expect(settings.sync('feishu')).resolves.toEqual(part)
  expect(store.blockRevision('policy')).toBe(store.blockRevision('policy'))
  run.mockRestore()
  expect(await settings.refreshLabels('feishu')).toEqual({ added: [] })
  vi.spyOn(Commands.prototype, 'lark').mockResolvedValueOnce({ chats: [
    { chat_id: 'oc_zulu', name: 'Zulu' }, { chat_id: 'oc_alpha', name: 'Alpha' }, { chat_id: 'oc_nameless' },
  ] })
  expect(await settings.refreshLabels('feishu')).toEqual({ added: ['Alpha', 'Zulu'] })
  expect(store.settings()).toEqual(settings.current())
})


it('resolves setup through worker probes and reports account and environment failures', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'superagent-setup-'))
  const bin = join(directory, 'bin'), state = join(directory, 'state')
  mkdirSync(bin); mkdirSync(state)
  const program = (file: string, body: string) => { writeFileSync(file, `#!/bin/sh\n${body}\n`); chmodSync(file, 0o755); return file }
  const gh = program(join(bin, 'gh'), 'case "$*" in\n'
    + '  "api user") echo \'{"login":"Agent-Bot","id":7}\' ;;\n'
    + '  "api repos/acme/widgets --jq {name: .full_name, push: .permissions.push}") echo \'{"name":"acme/widgets","push":true}\' ;;\n'
    + '  *) exit 1 ;;\nesac')
  const larkBody = (mode = 'bot') => `case "$*" in\n  "auth status") echo '{"appId":"cli_bot","identities":{"bot":{"status":"ready"}}}' ;;\n  "config strict-mode") echo 'strict-mode: ${mode} (source: profile)' ;;\n  "api GET /open-apis/bot/v3/info --as bot --format ndjson") echo '{"bot":{"open_id":"ou_bot","app_name":"Widgets Bot"}}' ;;\nesac`
  const lark = program(join(bin, 'lark-cli'), larkBody())
  let gitFails = false
  const runtime: SubprocessRuntime = {
    spawn: ({ argv, cwd }: { argv: readonly string[]; cwd: string }) => {
      const env: Record<string, string> = { ...process.env, PATH: process.env.PATH ?? '' }
      let index = 0
      if (argv[0] === 'env' && String(argv[1]).startsWith('PATH=')) {
        env.PATH = String(argv[1]).slice('PATH='.length)
        index = 2
      }
      while (index < argv.length && (argv[index] === 'env' || /^[A-Z0-9_]+=/.test(String(argv[index])))) {
        const assignment = String(argv[index])
        if (/^[A-Z0-9_]+=/.test(assignment)) env[assignment.slice(0, assignment.indexOf('='))] = assignment.slice(assignment.indexOf('=') + 1)
        index++
      }
      const command = String(argv[index]), args = argv.slice(index + 1).map(String)
      if (gitFails && command.endsWith('git')) {
        return { done: Promise.resolve({ exitCode: 1 }), collected: { stdout: reader(() => ''), stderr: reader(() => 'old git') } }
      }
      const output = { stdout: '', stderr: '' }
      const child = spawn(command, args, { cwd, env })
      child.stdout.on('data', (chunk) => { output.stdout += String(chunk) })
      child.stderr.on('data', (chunk) => { output.stderr += String(chunk) })
      return { done: new Promise(resolve => child.on('close', (code) => { resolve({ exitCode: code }) })), collected: { stdout: reader(() => output.stdout), stderr: reader(() => output.stderr) } }
    },
  } as unknown as SubprocessRuntime
  const reader = (text: () => string) => ({ readFrom: () => ({ text: text(), nextOffset: 0, lossy: false }) })
  const host = (restarts: number[] = []) => ({
    context: { subprocess: runtime, logger: { warn: vi.fn() } },
    machine: testMachine({ stateDir: state }),
    lifetime: new AbortController().signal,
    restart: () => { restarts.push(restarts.length) },
  })
  const settings = (overrides: Record<string, unknown> = {}) => SettingsSchema.parse({
    sources: { github: { enabled: true, config: { repository: 'acme/widgets', cli: gh } }, feishu: { enabled: true, config: { cli: lark } } },
    ...overrides,
  })
  const oldPath = process.env.PATH
  process.env.PATH = [bin, oldPath].join(':')
  try {
    const setup = new Setup(host())
    expect(() => setup.deployment).toThrow('waiting for setup: setup has not run')
    await setup.blockCommands(['blocked'])
    expect(await setup.run(settings())).toBe(true)
    expect(setup.deployment).toMatchObject({ repository: 'acme/widgets', github: { login: 'Agent-Bot' } })
    expect(setup.commandEnvironment.PATH?.split(':')).not.toContain(join(state, 'worker-bin'))
    await setup.blockCommands(['blocked'])
    expect(() => { setup.check() }).toThrow('Setup is complete')
    setup.restore(); setup.restore()

    writeFileSync(lark, `#!/bin/sh\n${larkBody('off')}\n`)
    const restarts: number[] = []
    const withoutFeishu = new Setup(host(restarts))
    expect(await withoutFeishu.run(settings())).toBe(true)
    expect(withoutFeishu.feishuProblem).toMatchObject({ place: 'feishu', code: 'feishuUserIdentity' })
    withoutFeishu.check()
    withoutFeishu.restore()
    await vi.waitFor(() =>{  expect(restarts).toEqual([0]) })

    const pending = new Setup(host())
    expect(await pending.run(settings({ sources: { github: { enabled: true, config: { repository: '', cli: gh } }, feishu: { enabled: true, config: { cli: lark } } } }))).toBe(false)
    expect(pending.problem).toMatchObject({ place: 'github', code: 'repositoryUnset' })
    await expect(pending.checkAccounts('source:github', settings({ sources: { github: { enabled: true, config: { repository: '', cli: gh } }, feishu: { enabled: true, config: { cli: lark } } } }))).rejects.toThrow('repository')
    await expect(new Setup(host()).checkAccounts('source:feishu', settings())).rejects.toThrow('strict mode')

    gitFails = true
    const oldGit = new Setup(host())
    expect(await oldGit.run(settings())).toBe(false)
    expect(oldGit.problem).toMatchObject({ code: 'gitTooOld' })
    gitFails = false

    const failedWrite = join(directory, 'failed-state')
    mkdirSync(failedWrite)
    writeFileSync(join(failedWrite, 'worker-bin'), '')
    const writeHost = host()
    ;(writeHost.machine as { stateDir: string }).stateDir = failedWrite
    const failed = new Setup(writeHost)
    expect(await failed.run(settings())).toBe(false)
    expect(failed.problem).toMatchObject({ code: 'environmentFailed', values: { output: expect.stringContaining('EEXIST') as unknown } })

    const invalidFeishu = settings({ sources: { github: { enabled: true, config: { repository: 'acme/widgets', cli: gh } },
      feishu: { enabled: true, config: { cli: 123 } } } })
    await expect(new Setup(host()).run(invalidFeishu)).rejects.toThrow()
    vi.mocked(writeEnvironment).mockRejectedValueOnce('worker environment failed')
    const nonError = new Setup(host())
    expect(await nonError.run(settings())).toBe(false)
    expect(nonError.problem).toMatchObject({ code: 'environmentFailed', values: { output: 'worker environment failed' } })
    delete process.env.PATH
    expect(new Setup(host()).commandEnvironment).toHaveProperty('PATH')
  } finally {
    if (oldPath !== undefined) process.env.PATH = oldPath
    rmSync(directory, { recursive: true, force: true })
  }
}, 20_000)
