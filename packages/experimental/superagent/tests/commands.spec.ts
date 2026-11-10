import { describe, expect, it, vi } from 'vitest'
import { z } from 'zod'
import { CliError, Commands, commandError } from '../src/infra/commands.ts'
import { feishuKind } from '../src/sources/feishu.ts'
import { feishuChannel } from '../src/sources/feishu-trigger.ts'
import { githubKind } from '../src/sources/github.ts'
import { githubChannel } from '../src/sources/github-trigger.ts'
import { sendGithub } from '../src/sources/github-channel.ts'
import { candidateCard, repliedTo, sourceLink } from '../src/app/links.ts'
import { TextCache } from '../src/app/cache.ts'
import { SourceRegistry } from '../src/sources/registry.ts'
import { SettingsSchema } from '../src/policy.ts'
import type { Candidate, Ticket, Trigger } from '../src/core/index.ts'
import { key, type OutboundMessage } from '../src/core/index.ts'
import { testDeployment } from './fixtures/deployment.ts'
const sources = new SourceRegistry(() => SettingsSchema.parse({}))
sources.register(feishuKind); sources.register(githubKind)
const send = (commands: Commands, message: OutboundMessage) => sources.send(message, commands)
const deployment = testDeployment({ github: { ...testDeployment().github, cli: '/path/agent-gh' } })
const REPOSITORY = deployment.repository
const replyBase = () => ({ id: 'stable-operation', channel: githubChannel(8), text: 'Done', kind: 'reply' as const })
const reply = replyBase()
describe('account-isolated operations', () => {
  it('uses the agent account\'s GitHub CLI and preserves literal shell metacharacters in argv', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal); const run = vi.spyOn(commands,'run').mockResolvedValue('{}')
    await commands.gh(['api','some-path','-f','body=`whoami` $(touch x)'])
    expect(run).toHaveBeenCalledWith('/path/agent-gh', ['api','some-path','-f','body=`whoami` $(touch x)'])
  })
  it('runs every command with its environment over the process environment, where undefined removes a variable', async () => {
    process.env.GH_TOKEN = 'personal'
    try {
      const output = await new Commands(() => deployment, { GH_TOKEN: undefined, DSH_SUPERAGENT_PROBE: 'agent' }, new AbortController().signal).run('env', [])
      expect(output).toContain('DSH_SUPERAGENT_PROBE=agent')
      expect(output).not.toContain('GH_TOKEN=')
    } finally { delete process.env.GH_TOKEN }
  })
  it('refuses Feishu commands when the deployment has no Feishu bot', async () => {
    await expect(new Commands(() => testDeployment({ feishu: undefined }), {}, new AbortController().signal).lark(['im', '+chat-list'])).rejects.toThrow('The Feishu Source has no working bot CLI')
  })
  it('reconciles an uncertain GitHub send with its stable marker', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal)
    vi.spyOn(commands,'pages').mockResolvedValue([{ body:'<!-- dsh-superagent:stable-operation -->',user:{ login:'agent-bot' } }])
    const gh = vi.spyOn(commands,'gh')
    await send(commands, reply); expect(gh).not.toHaveBeenCalled()
  })
  it('posts the reply as the model wrote it, only to the fixed repository', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal); vi.spyOn(commands,'pages').mockResolvedValue([])
    const gh = vi.spyOn(commands,'gh').mockResolvedValue({})
    await send(commands, reply)
    // The disclosure line is prompt text: the model writes it, and the host adds nothing.
    expect(gh).toHaveBeenCalledWith(['api',`repos/${REPOSITORY}/issues/8/comments`,'--method','POST','-f',expect.stringMatching(/^body=Done\n\n<!-- dsh-superagent:stable-operation -->$/)])
    await expect(new Commands(() => deployment, {}, new AbortController().signal).pages(z.unknown(),
      '../../someone/else/issues')).rejects.toThrow()
  })
  it('answers the review comment itself when it has no earlier reply, or falls back when GitHub deleted it', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal)
    const review = `https://github.com/${REPOSITORY}/pull/8#discussion_r7`
    const reply = { ...replyBase(), channel: githubChannel(8, review) }
    vi.spyOn(commands, 'pages').mockResolvedValue([])
    const api = vi.spyOn(commands, 'api')
    api.mockResolvedValueOnce({ id: 7 }).mockResolvedValueOnce({})
    await expect(sendGithub({ commands }, reply)).resolves.toBe('sent')
    expect(api.mock.calls[1]![1]).toBe('pulls/8/comments/7/replies')
    api.mockRejectedValueOnce(new CliError('deleted', 410)).mockResolvedValueOnce({})
    await expect(sendGithub({ commands }, reply)).resolves.toBe('sent')
    expect(api.mock.calls[3]![1]).toBe('issues/8/comments')
    api.mockRejectedValueOnce(new Error('network down'))
    await expect(sendGithub({ commands }, reply)).rejects.toThrow('network down')
    await expect(sendGithub({ commands }, { ...replyBase(), channel: { ...githubChannel(8), address: {} } })).rejects.toThrow('Missing GitHub reply target')
  })

  it('routes review-comment replies to the root review comment', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal); vi.spyOn(commands,'pages').mockResolvedValue([])
    const gh = vi.spyOn(commands,'gh').mockResolvedValueOnce({ id: 7, in_reply_to_id: 3 }).mockResolvedValueOnce({})
    await send(commands, { ...reply, channel: githubChannel(8, `https://github.com/${REPOSITORY}/pull/8#discussion_r7`) })
    expect(gh.mock.calls[1]![0]).toEqual(['api', `repos/${REPOSITORY}/pulls/8/comments/3/replies`, '--method', 'POST', '-f', expect.stringContaining('Done')])
  })
  it('uses Feishu bot identity and one idempotency key on retries', async () => {
    const commands = new Commands(() => deployment, {}, new AbortController().signal); const run = vi.spyOn(commands,'run').mockResolvedValue('{"ok":true,"data":{}}')
    await send(commands, { ...reply, channel: feishuChannel('oc_1', 'om_1') })
    expect(run.mock.calls[0]![1]).toEqual(expect.arrayContaining(['--as','bot','--idempotency-key','stable-operation','--message-id','om_1']))
  })
})

it('reports Feishu failures with their code and without command arguments', () => {
  const membership = commandError({ stderr:'[page 1] fetching...\n'+JSON.stringify({ error:{ code:232011,message:'operator is not in the chat' } }) })
  expect(membership).toMatchObject({ message: 'operator is not in the chat (232011)', code: 232011 })
  expect(commandError({ stderr:JSON.stringify({ error:{ code:999,message:'scope missing' } }) }).message).toBe('scope missing (999)')
})
describe('Channels', () => {
  const commands = () => new Commands(() => deployment, {}, new AbortController().signal)
  const feishu = { ...reply, channel: feishuChannel('oc_1', 'om_1') }
  const lark = (code: number, message: string) => commandError({ stderr: JSON.stringify({ error: { code, message } }), code: 1 })
  it('drop a Feishu reply to a recalled message or one Feishu no longer finds, and say how to add the bot to a chat', async () => {
    const value = commands()
    const run = vi.spyOn(value, 'run')
    for (const [code, message] of [[230011, 'The message is recalled'], [230001, 'message not found'], [231003,
      'The message is withdrawn']] as const) {
      run.mockRejectedValueOnce(lark(code, message))
      expect(await send(value, feishu)).toBe('gone')
    }
    run.mockRejectedValueOnce(lark(232011, 'operator is not in the chat'))
    const hint = 'The Feishu bot is not a member of the chat. Add the bot in the chat settings, then retry.'
    await expect(send(value, feishu)).rejects.toThrow(hint)
    run.mockRejectedValueOnce(lark(99991400, 'rate limited'))
    await expect(send(value, feishu)).rejects.toThrow('rate limited (99991400)')
  })
  it('keep a Feishu reply whose command timed out for a retry, whatever its text says', async () => {
    const value = commands()
    const timedOut = { killed: true, signal: 'SIGTERM', code: null, stderr: '', message: 'Command failed: lark-bot --markdown withdrawn' }
    vi.spyOn(value, 'run').mockRejectedValueOnce(commandError(timedOut, 'lark-bot'))
    await expect(send(value, { ...feishu, text: 'The change was withdrawn' })).rejects.toThrow('lark-bot timed out after 90 s')
  })
  it('drop a GitHub reply to an Issue that is deleted or locked, and keep others for a retry', async () => {
    const value = commands()
    vi.spyOn(value, 'pages').mockResolvedValue([])
    const gh = vi.spyOn(value, 'gh')
    for (const stderr of ['gh: This issue was deleted (HTTP 410)', 'gh: Unable to create comment because issue is locked. (HTTP 403)']) {
      gh.mockRejectedValueOnce(commandError({ stderr, code: 1 }))
      expect(await send(value, reply)).toBe('gone')
    }
    // GitHub also answers 404 for a private repository the agent lost access to, so 404 is retried rather than dropped.
    for (const stderr of ['gh: Not Found (HTTP 404)', 'gh: Server Error (HTTP 500)']) {
      gh.mockRejectedValueOnce(commandError({ stderr, code: 1 }))
      await expect(send(value, reply)).rejects.toThrow(stderr)
    }
  })
  it('reply on the PR\'s conversation when the review comment a reply points at is gone', async () => {
    const value = commands()
    const pages = vi.spyOn(value, 'pages').mockResolvedValue([])
    const gh = vi.spyOn(value, 'gh')
      .mockRejectedValueOnce(commandError({ stderr: 'gh: Not Found (HTTP 404)', code: 1 })).mockResolvedValueOnce({})
    expect(await send(value, { ...reply, channel: githubChannel(8, `https://github.com/${REPOSITORY}/pull/8#discussion_r7`) })).toBe('sent')
    expect((pages.mock.calls[0] as unknown[])[1]).toBe('issues/8/comments')
    const post = ['api', `repos/${REPOSITORY}/issues/8/comments`, '--method', 'POST', '-f', expect.stringContaining('Done')]
    expect(gh.mock.calls[1]![0]).toEqual(post)
  })
  it('link each address to its page with its label', () => {
    const links = { repository: REPOSITORY, feishu: deployment.feishu! }
    expect(githubKind.channel!.link(githubChannel(8).address, links)).toEqual({ url: `https://github.com/${REPOSITORY}/issues/8`,
      label: { zh: 'GitHub #8', en: 'GitHub #8' } })
    const review = `https://github.com/${REPOSITORY}/pull/8#discussion_r7`
    expect(githubKind.channel!.link(githubChannel(8, review).address, links))
      .toEqual({ url: review, label: { zh: 'GitHub #8 的 review 讨论', en: 'Review discussion on GitHub #8' } })
    expect(githubKind.channel!.link(githubChannel(8).address, {}).url).toBe('')
    const message = feishuChannel('oc_1', 'om_1').address
    expect(feishuKind.channel!.link(message, links)).toEqual({ url: 'https://applink.feishu.cn/client/chat/open?openChatId=oc_1',
      label: { zh: '飞书消息', en: 'Feishu message' }, triggerId: key('feishu', 'om_1') })
  })
})

describe('GitHub calls', () => {
  const commands = () => new Commands(() => deployment, {}, new AbortController().signal)
  it('go only to paths inside the configured repository, and validate what GitHub answers', async () => {
    const value = commands()
    const run = vi.spyOn(value, 'run').mockResolvedValue('{"number":12,"title":"Login fails","extra":true}')
    expect(await value.api(z.looseObject({ number: z.number() }), 'issues/12')).toEqual({ number: 12, title: 'Login fails', extra: true })
    expect(run).toHaveBeenLastCalledWith('/path/agent-gh', ['api', `repos/${REPOSITORY}/issues/12`])
    await expect(value.api(z.looseObject({ number: z.string() }), 'issues/12')).rejects.toThrow()
    for (const path of ['../other/repo/issues', 'issues/../../x', '/repos/x/y', 'https://api.github.com/x']) {
      await expect(value.api(z.unknown(), path)).rejects.toThrow('restricted to the configured repository')
    }
    expect(run).toHaveBeenCalledTimes(2)
    // An empty path targets the repository root.
    await value.api(z.unknown(), '')
    expect(run).toHaveBeenLastCalledWith('/path/agent-gh', ['api', `repos/${REPOSITORY}`])
    // Without a schema, api() returns the raw CLI stdout.
    run.mockResolvedValue('raw file')
    expect(await value.api('contents/README.md', '-H', 'Accept: application/vnd.github.raw')).toBe('raw file')
  })
  it('read every page of a list, and skip and count an item that does not validate', async () => {
    const value = commands()
    const run = vi.spyOn(value, 'run').mockResolvedValue('[[{"id":1}],[{"id":2}]]')
    expect(await value.pages(z.looseObject({ id: z.number() }), 'issues/12/comments?per_page=100')).toEqual([{ id: 1 }, { id: 2 }])
    expect(run).toHaveBeenLastCalledWith('/path/agent-gh', ['api', `repos/${REPOSITORY}/issues/12/comments?per_page=100`, '--paginate',
      '--slurp'])
    run.mockResolvedValue('[[{"id":1}],[{"id":"two"}]]')
    const tally = { skipped: 0 }
    expect(await value.pages(z.looseObject({ id: z.number() }), 'issues/12/comments', tally)).toEqual([{ id: 1 }])
    expect(tally).toEqual({ skipped: 1 })
    run.mockResolvedValue('{"message":"not a list"}')
    await expect(value.pages(z.looseObject({ id: z.number() }), 'issues/12/comments')).rejects.toThrow()
  })
})
it('says how to add the bot to a chat when Feishu answers that it is not a member, and keeps the code', async () => {
  const commands = new Commands(() => deployment, {}, new AbortController().signal)
  const membership = JSON.stringify({ error: { code: 232011, message: 'operator is not in the chat' } })
  const run = vi.spyOn(commands, 'run').mockRejectedValueOnce(commandError({ stderr: membership, code: 1 }))
  const hint = 'The Feishu bot is not a member of the chat. Add the bot in the chat settings, then retry.'
  await expect(commands.lark(['im', '+chat-members-list'])).rejects.toMatchObject({ message: hint, code: 232011, exitCode: 1 })
  run.mockResolvedValueOnce(JSON.stringify({ ok: false, error: { code: 232011, message: 'operator is not in the chat' } }))
  await expect(commands.lark(['im', '+chat-members-list'])).rejects.toMatchObject({ message: hint, code: 232011 })
})
it('names only the program and how it ended when a command prints no diagnostics, never its arguments', async () => {
  const commands = new Commands(() => deployment, {}, new AbortController().signal)
  const secret = 'secret reply text'
  await expect(commands.run('sh', ['-c', 'exit 3', secret])).rejects.toMatchObject({ message: 'sh exited with code 3', exitCode: 3 })
  await expect(commands.run('sh', ['-c', 'kill -9 $$', secret])).rejects.toMatchObject({ message: 'sh was stopped by SIGKILL' })
  await expect(commands.run('/no/such/program', ['secret reply text'])).rejects.toMatchObject({ message: 'program could not run: ENOENT' })
  // Timeouts kill the child with SIGTERM; Node's default error message would echo every argument.
  const timedOut = { killed: true, signal: 'SIGTERM', code: null, stderr: '',
    message: 'Command failed: /path/lark-bot im +messages-reply --markdown withdrawn' }
  expect(commandError(timedOut, '/path/lark-bot')).toMatchObject({ message: 'lark-bot timed out after 90 s' })
  expect(String(commandError(timedOut, '/path/lark-bot'))).not.toContain('withdrawn')
})
it('reports failures as CliError with the code the CLI names and the exit status', async () => {
  const notFound = commandError({ stderr: 'gh: Not Found (HTTP 404)\n', code: 1 })
  expect(notFound).toBeInstanceOf(CliError)
  expect(notFound).toMatchObject({ message: 'gh: Not Found (HTTP 404)', code: 404, exitCode: 1 })
  expect(commandError({ stderr: JSON.stringify({ error: { code: 230011, message: 'The message is recalled' } }),
    code: 1 })).toMatchObject({ message: 'The message is recalled (230011)', code: 230011 })
  // A cancelled command stays an AbortError so callers can distinguish cancellation from failure.
  const aborted = Object.assign(new Error('The operation was aborted'), { name: 'AbortError', stderr: '' })
  expect(commandError(aborted)).toBe(aborted)
  const controller = new AbortController()
  const running = new Commands(() => deployment, {}, controller.signal).run('sleep', ['5'])
  controller.abort()
  await expect(running).rejects.toMatchObject({ name: 'AbortError' })
  const commands = new Commands(() => deployment, {}, new AbortController().signal)
  vi.spyOn(commands, 'run').mockResolvedValue(JSON.stringify({ ok: false, error: { code: 231003, message: 'message not found' } }))
  await expect(commands.lark(['im', '+messages-reply'])).rejects.toMatchObject({ message: 'message not found (231003)', code: 231003 })
})

it('runs git through the agent environment and reports every command ending without argv', async () => {
  const commands = new Commands(() => deployment, {}, new AbortController().signal)
  const run = vi.spyOn(commands, 'run').mockResolvedValue('branch')
  expect(await commands.git(['status'], '/repo')).toBe('branch')
  expect(run).toHaveBeenCalledWith('git', ['status'], '/repo')
  expect(commandError({ stderr: JSON.stringify({ error: { code: 999 } }), code: 1 }).message).toBe('{"error":{"code":999}}')
  expect(commandError({ code: 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER' }).message).toBe('command printed more output than it may')
  expect(commandError({ killed: true, signal: 'SIGKILL' }).message).toBe('command was stopped by SIGKILL')
  expect(commandError(undefined).message).toBe('command failed')
  const lark = new Commands(() => deployment, {}, new AbortController().signal)
  vi.spyOn(lark, 'run').mockResolvedValue('{"ok":false}')
  await expect(lark.lark(['im', '+chat-list'])).rejects.toMatchObject({ message: 'Feishu command failed' })
})

it('stops a running command when the signal of its work aborts, or the lifetime, and leaves the other work running', async () => {
  const lifetime = new AbortController(), work = new AbortController()
  const commands = new Commands(() => deployment, {}, lifetime.signal)
  const started = Date.now()
  const running = commands.withSignal(work.signal).run('sleep', ['5'])
  work.abort()
  await expect(running).rejects.toThrow()
  // Only commands bound to the aborted work signal stop.
  expect(await commands.run('echo', ['still'])).toBe('still\n')
  const other = commands.withSignal(new AbortController().signal).run('sleep', ['5'])
  lifetime.abort()
  await expect(other).rejects.toThrow()
  expect(Date.now() - started).toBeLessThan(4000)
})

const trigger = (id: string, overrides: Partial<Trigger> = {}): Trigger => ({
  id, source: 'github', threads: [{ source: 'github', id: '12' }], actor: { source: 'github', id: 'alice' },
  human: true, at: '2026-11-10T00:00:00Z', text: id, meta: {}, ...overrides,
})
const ticket = (id: string): Ticket => ({
  id, title: `Ticket ${id}`, decisionId: `decision-${id}`, mandate: { strength: 'explicit', policyVersion: 'v', basis: { via: 'listed' }, actor: { source: 'github', id: 'alice' }, scope: '' },
  channels: [], assignments: [], inputs: [], undelivered: [], createdAt: '2026-11-10T00:00:00Z', updatedAt: '2026-11-10T00:00:00Z',
  origin: { triggerId: 'primary', triggerIds: ['primary'] },
})
const commands = () => new Commands(() => testDeployment(), {}, new AbortController().signal)
const linkSources = (find: (query: string) => { threads: string[]; terms: string[] } = () => ({ threads: [], terms: [] })) =>
  ({ instance: () => ({ id: 'feishu', kind: 'feishu', enabled: true, config: { linkTimeoutMs: 10 } }), find, repliesTo: () => 'parent' }) as unknown as SourceRegistry
const linkHost = (overrides: Record<string, unknown> = {}) => ({
  store: {
    settings: () => SettingsSchema.parse({}),
    triggers: { get: (id: string) => id === 'parent' ? trigger('parent', { text: 'parent message' }) : undefined, disposition: undefined },
    threads: { get: undefined },
    tickets: { get: undefined },
  },
  links: new TextCache(100),
  commands: commands(),
  sources: linkSources(),
  lifetime: new AbortController().signal,
  ...overrides,
}) as Parameters<typeof sourceLink>[0]

it('resolves trigger URLs, cache entries, Feishu links, and readable fallbacks', async () => {
  const lark = vi.spyOn(Commands.prototype, 'lark')
  try {
    expect(await sourceLink(linkHost(), trigger('url', { url: 'https://example.com/message' }))).toEqual({ source_url: 'https://example.com/message' })
    expect(await sourceLink(linkHost(), trigger('github'))).toEqual({})
    const cached = linkHost({ links: new TextCache(100) })
    cached.links.set('oc_1:om_1', 'https://example.com/cached')
    const feishu = trigger('feishu', { source: 'feishu', meta: { chatId: 'oc_1', messageId: 'om_1' } })
    expect(await sourceLink(cached, feishu)).toEqual({ source_url: 'https://example.com/cached' })
    lark.mockResolvedValueOnce({ messages: [{ message_id: 'om_1', chat_id: 'oc_1', message_app_link: 'https://example.com/live' }] })
    const fresh = linkHost()
    expect(await sourceLink(fresh, feishu)).toEqual({ source_url: 'https://example.com/live' })
    expect(fresh.links.get('oc_1:om_1')).toBe('https://example.com/live')
    lark.mockRejectedValueOnce(new Error('link unavailable'))
    expect(await sourceLink(linkHost(), feishu)).toEqual({ source_url_read: 'feishu/feishu/message' })
    const lifetime = new AbortController(); lifetime.abort()
    lark.mockRejectedValueOnce(new Error('link unavailable'))
    await expect(sourceLink(linkHost({ lifetime: lifetime.signal }), feishu)).rejects.toThrow('aborted')
    const work = new AbortController(); work.abort()
    lark.mockRejectedValueOnce(new Error('link unavailable'))
    await expect(sourceLink(linkHost(), feishu, work.signal)).rejects.toThrow('aborted')
  } finally { lark.mockRestore() }
})

it("finds a Trigger's parent only when its Source names one the Store still has", () => {
  const reply = trigger('reply', { meta: { parentId: 'parent' } })
  const parent = trigger('parent', { text: 'parent message' })
  expect(repliedTo({ store: { triggers: { get: () => parent } }, sources: { repliesTo: () => 'parent' } }, reply)).toEqual(parent)
  expect(repliedTo({ store: { triggers: { get: () => parent } }, sources: { repliesTo: () => undefined } }, reply)).toBeUndefined()
  expect(repliedTo({ store: { triggers: { get: () => undefined } }, sources: { repliesTo: () => 'missing' } }, reply)).toBeUndefined()
})

it('builds candidate cards with batches, handbacks, delivery, references, and both routing actions', async () => {
  const primary = trigger('primary', { text: 'See #12 and #13 please' })
  const earlier = trigger('earlier', { text: 'earlier message' })
  const known = ticket('known'), referenced = ticket('referenced')
  const host = linkHost({
    store: {
      settings: () => SettingsSchema.parse({ eval: { mode: 'auto' } }),
      triggers: {
        get: (id: string) => id === 'parent' ? trigger('parent', { text: 'parent message' }) : undefined,
        disposition: (id: string) => id === 'primary'
          ? { fanout: ['worker'], handbacks: [{ ticketId: 'worker', reason: 'outside scope', at: '' }], at: '' }
          : undefined,
      },
      threads: { get: (key: string) => key === 'github:12' ? { subscriptions: ['known', 'referenced', 'missing'] } : undefined },
      tickets: { get: (id: string) => id === 'known' ? known : id === 'referenced' ? referenced : undefined },
    },
    sources: linkSources(query => ({ threads: query === '#12' ? ['github:12'] : ['github:13'], terms: [] })),
  })
  const candidate: Candidate = {
    evalId: 'eval', thread: { key: 'github:12', ref: { source: 'github', id: '12' }, createdAt: '', subscriptions: [], correlated: [], pending: [], meta: {} },
    triggers: [earlier, primary], strength: 'explicit', policyVersion: 'v', tickets: [known],
  }
  const card = await candidateCard(host, candidate)
  expect(card).toMatchObject({ trigger_id: 'primary', mode: 'auto', strength: 'explicit' })
  expect(card.batch).toHaveLength(1)
  expect(card.batch[0]).toMatchObject({ trigger_id: 'earlier', actor: 'alice', at: '2026-11-10T00:00:00Z', preview: 'earlier message' })
  expect(card.handed_back).toEqual([{ trigger_id: 'primary', ticket_id: 'worker', reason: 'outside scope' }])
  expect(card.related_tickets.map(item => item.ticket_id)).toEqual(['known'])
  expect(card.delivered_to).toEqual(['worker'])
  expect(card.referenced_tickets.map(item => item.ticket_id)).toEqual(['referenced'])
  expect(card.next_action).toContain('orchestrator_route')
  candidate.strength = 'inferred'
  expect((await candidateCard(host, candidate)).next_action).toContain('orchestrator_eval')
})
