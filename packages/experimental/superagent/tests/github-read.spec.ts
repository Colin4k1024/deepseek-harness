import { afterEach, expect, it, vi } from 'vitest'
import { Commands } from '../src/infra/commands.ts'
import { testDeployment } from './fixtures/deployment.ts'
import { ciFailures, ciSummary, githubThreadContext, pullChecks, readGithub, readRepositoryPath } from '../src/sources/github-read.ts'
import type { Trigger } from '../src/core/index.ts'

const commands = new Commands(() => testDeployment(), {}, new AbortController().signal)
afterEach(() => vi.restoreAllMocks())

it('reads a bounded comment page and exposes the next page reference', async () => {
  const gh = vi.spyOn(commands, 'gh').mockResolvedValue(Array.from({ length: 20 }, (_, id) => ({ id, body: 'comment' })))
  expect(await readGithub(commands, 'github/issues/123/comments/2')).toMatchObject({ next_resource: 'github/issues/123/comments/3' })
  expect(gh).toHaveBeenCalledWith(['api', 'repos/acme/widgets/issues/123/comments?per_page=20&page=2'])
})

it('rejects unsupported or escaping paths without making a request', async () => {
  const gh = vi.spyOn(commands, 'gh')
  for (const resource of ['github/../outside', 'github/issues/1/reviews', 'github/pulls/1/checks/2', 'github/issues/1?repo=other']) {
    await expect(readGithub(commands, resource)).rejects.toThrow()
  }
  expect(gh).not.toHaveBeenCalled()
})

it('reads checks across pages and canonicalizes the latest status per context', async () => {
  vi.spyOn(commands, 'gh').mockResolvedValue([{ check_runs: [{ name: 'z', status: 'completed', conclusion: 'success' }] }, { check_runs: [{ name: 'a', status: 'in_progress', conclusion: null }] }])
  vi.spyOn(commands, 'pages').mockResolvedValue([{ context: 'build', state: 'success' }, { context: 'build', state: 'failure' }])
  const result = await pullChecks(commands, 'abc')
  expect(result.checks.map((check: { name: string }) => check.name)).toEqual(['a', 'z'])
  expect(result.statuses).toEqual([{ context: 'build', state: 'success' }])
})

it('reads any requested PR’s CI on demand without restricting the author', async () => {
  const gh = vi.spyOn(commands, 'gh').mockResolvedValueOnce({ head: { sha: 'abc' }, user: { login: 'another-author' } }).mockResolvedValueOnce([{ check_runs: [] }])
  vi.spyOn(commands, 'pages').mockResolvedValue([])
  expect(await readGithub(commands, 'github/pulls/123/checks')).toEqual({ head: 'abc', checks: [], statuses: [] })
  expect(gh).toHaveBeenCalledTimes(2)
})

/** Fake GitHub REST API pull request review comment payload. */
const reviewComment = (id: number, reply?: number) => ({ id, url: `https://api.github.com/repos/acme/widgets/pulls/comments/${id}`,
  pull_request_review_id: 77, node_id: 'PRRC_x', path: 'src/render.ts', line: 43, original_line: 43, side: 'RIGHT', commit_id: 'abc',
  diff_hunk: '@@ -40,6 +40,8 @@\n   const title = view.title\n   const margin = 12\n+  const padding = 8',
  user: { login: id === 2 ? 'review-bot[bot]' : 'carol', id: 5, type: id === 2 ? 'Bot' : 'User', avatar_url: 'https://avatars/5', html_url: 'https://github.com/carol' },
  body: `comment ${id}`, created_at: `2026-09-09T00:00:0${id}Z`, updated_at: `2026-09-09T00:00:0${id}Z`,
  html_url: `https://github.com/acme/widgets/pull/9#discussion_r${id}`, author_association: 'MEMBER', reactions: { total_count: 0 },
  _links: { self: { href: 'x' } }, ...reply ? { in_reply_to_id: reply } : {} })
it('reads a page of review comments as messages: id, author, time, text, the comment each answers, and the file and line', async () => {
  vi.spyOn(commands, 'gh').mockResolvedValue([reviewComment(1), reviewComment(2, 1)])
  expect(await readGithub(commands, 'github/pulls/9/comments/1')).toEqual({ messages: [
    { id: '1', from: 'carol', at: '2026-09-09T00:00:01Z', text: 'comment 1', url: 'https://github.com/acme/widgets/pull/9#discussion_r1',
      path: 'src/render.ts', line: 43 },
    { id: '2', from: 'review-bot[bot]', at: '2026-09-09T00:00:02Z', text: 'comment 2', bot: true, reply_to: '1',
      url: 'https://github.com/acme/widgets/pull/9#discussion_r2', path: 'src/render.ts', line: 43 }] })
})
it('reads a review thread from any of its comments, the first with its file, line and diff lines', async () => {
  vi.spyOn(commands, 'pages').mockResolvedValue([reviewComment(3), reviewComment(2, 1), reviewComment(1), reviewComment(4, 3)].map(item => ({
    ...item, user: { login: item.user.login, type: item.user.type } })))
  const thread = await readGithub(commands, 'github/pulls/9/thread/2') as { messages: { id: string; text: string; reply_to?: string }[] }
  expect(thread.messages.map(message => [message.id, message.reply_to])).toEqual([['1', undefined], ['2', '1']])
  expect(thread.messages[0]!.text).toBe('src/render.ts:43\n   const title = view.title\n   const margin = 12\n+  const padding = 8\n\ncomment 1')
  await expect(readGithub(commands, 'github/issues/9/thread/2')).rejects.toThrow('require a pull request')
})
it('reads an Issue or PR as its author, time, title, state, description and a PR\'s branches', async () => {
  vi.spyOn(commands, 'gh').mockResolvedValue({ number: 9, id: 1, title: 'Fix the margin', state: 'open', body: 'Details', draft: true,
    user: { login: 'carol', type: 'User' }, created_at: '2026-09-09T00:00:00Z', html_url: 'https://github.com/acme/widgets/pull/9',
    head: { ref: 'agent/fix', sha: 'abc', repo: {} }, base: { ref: 'main' }, labels: [{ name: 'ui', color: 'fff' }], node_id: 'x', _links: {} })
  expect(await readGithub(commands, 'github/pulls/9')).toEqual({ number: 9, title: 'Fix the margin', state: 'open', draft: true, head: 'agent/fix',
    base: 'main', labels: ['ui'], id: '9', from: 'carol', at: '2026-09-09T00:00:00Z', text: 'Details', url: 'https://github.com/acme/widgets/pull/9' })
})


it('abbreviates a CI summary after five names', () => {
  const checks = Array.from({ length: 6 }, (_, index) => ({ name: `check-${index + 1}`, status: 'completed', conclusion: 'failure' }))
  expect(ciFailures({ head: 'abcdef123456', checks, statuses: [{ context: 'deploy', state: 'error' }] })).toEqual(['check-1', 'check-2', 'check-3', 'check-4', 'check-5', 'check-6', 'deploy'])
  expect(ciSummary({ head: 'abcdef123456', checks, statuses: [{ context: 'deploy', state: 'error' }] }, 'main', 'github/commits/abcdef123456/checks'))
    .toContain('check-1, check-2, check-3, check-4, check-5, and 2 more')
})

it('reads repository files and directory listings, hiding dot files and node_modules', async () => {
  const api = vi.spyOn(commands, 'api')
  api.mockResolvedValueOnce('file text')
  await expect(readRepositoryPath(commands, 'src/example.ts', 'file')).resolves.toBe('file text')
  expect(api.mock.calls[0]).toEqual(['contents/src/example.ts', '-H', 'Accept: application/vnd.github.raw'])

  api.mockResolvedValueOnce([{ name: 'z.ts' }, { name: '.hidden' }, { name: 'node_modules' }, { name: 'a.ts' }])
  await expect(readRepositoryPath(commands, './src', 'directory')).resolves.toBe('a.ts\nz.ts')

  for (const path of ['../outside', '.github/workflows', 'node_modules']) {
    await expect(readRepositoryPath(commands, path, 'file')).rejects.toThrow('Only non-hidden repository source paths are available')
  }
  api.mockResolvedValueOnce({})
  await expect(readRepositoryPath(commands, 'src', 'directory')).rejects.toThrow('src is not a directory')
})

it('reads commit checks directly and rejects incomplete thread requests', async () => {
  const gh = vi.spyOn(commands, 'gh').mockResolvedValue([{ check_runs: [] }])
  vi.spyOn(commands, 'pages').mockResolvedValue([{ context: 'build', state: 'success' }, { context: 'deploy', state: 'success' }])
  await expect(readGithub(commands, 'github/commits/abcdef123456/checks')).resolves.toEqual({ head: 'abcdef123456', checks: [], statuses: [{ context: 'build', state: 'success' }, { context: 'deploy', state: 'success' }] })
  await expect(readGithub(commands, 'github/pulls/9/thread')).rejects.toThrow('Name the review thread')
  vi.spyOn(commands, 'pages').mockResolvedValue([])
  await expect(readGithub(commands, 'github/pulls/9/thread/404')).rejects.toThrow('No review comment')
  expect(gh).toHaveBeenCalledWith(['api', 'repos/acme/widgets/commits/abcdef123456/check-runs?per_page=100', '--paginate', '--slurp'])
})

it('reads issue and PR descriptions with optional branches, labels, and bot authors', async () => {
  const api = vi.spyOn(commands, 'api')
  const issue = { id: 1, number: 9, title: 'Fix', state: 'closed', body: null, user: { login: 'policy-bot', type: 'Bot' },
    created_at: '2026-09-09T00:00:00Z', html_url: 'https://github.com/acme/widgets/issues/9', labels: [] }
  api.mockResolvedValueOnce(issue)
  await expect(readGithub(commands, 'github/issues/9')).resolves.toEqual({ number: 9, title: 'Fix', state: 'closed',
    id: '9', from: 'policy-bot', at: '2026-09-09T00:00:00Z', text: '', bot: true,
    url: 'https://github.com/acme/widgets/issues/9' })

  const pull = { ...issue, body: 'Details', user: { login: 'carol', type: 'User' }, merged: true, draft: false,
    labels: [{ name: 'ui' }], head: { ref: 'agent/fix' }, base: { ref: 'main' } }
  api.mockResolvedValueOnce(pull)
  await expect(readGithub(commands, 'github/pulls/9')).resolves.toMatchObject({ merged: true, head: 'agent/fix', base: 'main', labels: ['ui'] })
})

it('reads reviews and comment pages, including absent optional fields and unreadable records', async () => {
  const api = vi.spyOn(commands, 'api')
  api.mockResolvedValueOnce([{ id: 2, user: { login: 'review-bot[bot]', type: 'Bot' }, state: 'CHANGES_REQUESTED', submitted_at: '2026-09-09T00:00:02Z' }])
  await expect(readGithub(commands, 'github/pulls/9/reviews')).resolves.toEqual({ messages: [{
    id: '2', from: 'review-bot[bot]', at: '2026-09-09T00:00:02Z', text: 'CHANGES_REQUESTED', bot: true }] })

  const comment = { id: 3, user: { login: 'carol' }, created_at: null }
  api.mockResolvedValueOnce([...Array.from({ length: 18 }, (_, index) => ({ id: index + 10, body: 'ok', user: { login: 'carol' } })), comment, {}])
  const page = await readGithub(commands, 'github/issues/9/comments/2') as {
    messages: { id: string; from: string; at: string; text: string }[]
    skipped: number
    next_resource: string
  }
  expect(page.messages).toHaveLength(19)
  expect(page.messages.at(-1)).toEqual({ id: '3', from: 'carol', at: '', text: '' })
  expect(page.skipped).toBe(1)
  expect(page.next_resource).toBe('github/issues/9/comments/3')
})

it('reads a Trigger conversation by page, putting the description first and using ids without URLs', async () => {
  const api = vi.spyOn(commands, 'api')
  const issue = { id: 1, number: 9, title: 'Fix', state: 'open', body: null, user: { login: 'carol' },
    created_at: '2026-09-09T00:00:00Z', html_url: 'https://github.com/acme/widgets/issues/9' }
  const comment = { id: 4, body: null, user: { login: 'carol' }, created_at: '2026-09-09T00:00:01Z' }
  api.mockResolvedValueOnce(issue).mockResolvedValueOnce([...Array.from({ length: 98 }, (_, index) => ({
    id: index + 10, body: 'ok', user: { login: 'carol' }, html_url: `https://github.com/x#${index}` })), comment, {}])
  const trigger: Trigger = { id: 'trigger', source: 'github', threads: [{ source: 'github', id: '9' }],
    actor: { source: 'github', id: 'carol' }, human: true, at: '2026-09-09T00:00:00Z', text: 'Fix', meta: { number: 9 } }
  const first = await githubThreadContext.read(commands, trigger, 'thread')
  expect(first.messages).toHaveLength(100)
  expect([first.messages[0], first.messages.at(-1)].map(message => [message?.id, message?.text])).toEqual([
    ['https://github.com/acme/widgets/issues/9', 'Fix\n\n'], ['4', ''],
  ])
  expect(first.next).toBe('2')

  api.mockResolvedValueOnce([{ id: 5, body: 'reply', user: { login: 'carol' }, created_at: '2026-09-09T00:00:02Z' }])
  const second = await githubThreadContext.read(commands, trigger, 'thread', '2')
  expect(second.messages).toEqual([{ id: '5', from: 'carol', at: '2026-09-09T00:00:02Z', text: 'reply' }])
  expect(second.next).toBeUndefined()
})

it('reads default-branch CI as its own context when its Trigger has no URL', async () => {
  const trigger: Trigger = { id: 'ci-trigger', source: 'github', threads: [{ source: 'github', id: 'ci:main' }],
    actor: { source: 'github', id: '' }, human: false, at: '2026-09-09T00:00:00Z', text: 'CI failed', meta: { branch: 'main' } }
  expect(githubThreadContext.messageId(trigger)).toBe('')
  await expect(githubThreadContext.read(commands, trigger, 'thread')).resolves.toEqual({
    messages: [{ id: 'ci-trigger', from: 'CI', at: '2026-09-09T00:00:00Z', text: 'CI failed', bot: true }],
    body: { status: 'branch_ci', messages: [{ id: 'ci-trigger', from: 'CI', at: '2026-09-09T00:00:00Z', text: 'CI failed', bot: true }] },
    status: 'branch_ci' })
})


it('reads a review with only a comment state and one whose time is its creation time', async () => {
  const api = vi.spyOn(commands, 'api')
  api.mockResolvedValueOnce([
    { id: 1, user: { login: 'carol' }, state: 'COMMENTED', body: 'looks good', submitted_at: '2026-09-09T00:00:01Z' },
    { id: 2, user: { login: 'carol' }, state: 'CHANGES_REQUESTED', body: null, created_at: '2026-09-09T00:00:02Z' },
  ])
  await expect(readGithub(commands, 'github/pulls/9/reviews')).resolves.toEqual({ messages: [
    { id: '1', from: 'carol', at: '2026-09-09T00:00:01Z', text: 'looks good' },
    { id: '2', from: 'carol', at: '2026-09-09T00:00:02Z', text: 'CHANGES_REQUESTED' }] })
})

it('reads review threads whose comments have no line, body, or creation time', async () => {
  vi.spyOn(commands, 'pages').mockResolvedValue([
    { id: 1, user: { login: 'review-bot[bot]', type: 'Bot' }, path: 'src/render.ts' },
    { id: 2, user: { login: 'carol' }, in_reply_to_id: 1, body: null, created_at: '2026-09-09T00:00:02Z' },
  ])
  const result = await readGithub(commands, 'github/pulls/9/thread/2') as { messages: { id: string; line?: number; text: string; reply_to?: string }[] }
  expect(result.messages).toEqual([
    { id: '1', from: 'review-bot[bot]', at: '', text: 'src/render.ts', bot: true, path: 'src/render.ts' },
    { id: '2', from: 'carol', at: '2026-09-09T00:00:02Z', text: '', reply_to: '1' },
  ])
  expect(result.messages[0]).not.toHaveProperty('line')
})

it('reads a conversation for an inline Trigger whose comment id is not numeric', async () => {
  const api = vi.spyOn(commands, 'api')
  const issue = { id: 1, number: 9, title: 'Fix', state: 'open', body: 'Details', user: { login: 'carol' },
    created_at: '2026-09-09T00:00:00Z', html_url: 'https://github.com/acme/widgets/issues/9' }
  const listed = [{ id: 4, body: 'reply', user: { login: 'carol' }, created_at: '2026-09-09T00:00:01Z' }, {}]
  api.mockResolvedValueOnce(issue).mockResolvedValueOnce(listed)
  const trigger: Trigger = { id: 'trigger', source: 'github', threads: [{ source: 'github', id: '9' }],
    actor: { source: 'github', id: 'carol' }, human: true, at: '2026-09-09T00:00:00Z', text: 'Fix',
    meta: { number: 9, kind: 'inline', commentId: '4' } }
  await expect(githubThreadContext.read(commands, trigger, 'thread')).resolves.toMatchObject({
    messages: [{ id: 'https://github.com/acme/widgets/issues/9', text: 'Fix\n\nDetails' }, { id: '4', text: 'reply' }],
    body: { skipped: 1 }, next: undefined })
})

it('orders a review thread when GitHub omits comment timestamps', async () => {
  const missingTime = (id: number, reply?: number) => {
    const item = reviewComment(id, reply)
    delete item.created_at
    delete item.updated_at
    return item
  }
  vi.spyOn(commands, 'pages').mockResolvedValue([missingTime(2, 1), missingTime(1)])
  const thread = await readGithub(commands, 'github/pulls/9/thread/1') as { messages: { id: string }[] }
  expect(thread.messages.map(message => message.id)).toEqual(['2', '1'])
})

it('uses a review comment id when GitHub omits its URL', async () => {
  const comment = { ...reviewComment(1), html_url: undefined }
  vi.spyOn(commands, 'pages').mockResolvedValue([comment])
  const trigger: Trigger = { id: 'inline', source: 'github', threads: [{ source: 'github', id: '9' }],
    actor: { source: 'github', id: 'carol' }, human: true, at: '2026-09-09T00:00:00Z', text: 'comment',
    meta: { kind: 'inline', commentId: 1, number: 9 } }
  await expect(githubThreadContext.read(commands, trigger, 'thread')).resolves.toMatchObject({
    messages: [{ id: '1' }],
  })
})
