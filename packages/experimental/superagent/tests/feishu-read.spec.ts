import { afterEach, expect, it, vi } from 'vitest'
import type { Agent } from '@deepseek-ai/dsh-agent'
import { Commands } from '../src/infra/commands.ts'
import { testDeployment } from './fixtures/deployment.ts'
import { feishuRead, feishuReferences, feishuThreadContext, readFeishu, readFeishuLink } from '../src/sources/feishu-read.ts'
import { feishuTrigger } from '../src/sources/feishu.ts'
import { readAdmissionImage } from '../src/sources/feishu-image.ts'
import type { FeishuRef } from '../src/sources/feishu-read.ts'
vi.mock('../src/sources/feishu-image.ts', () => ({
  admissionImageKeys: (text: string) => [...new Set(text.match(/\bimg_[a-zA-Z0-9_-]+\b/g) ?? [])],
  readAdmissionImage: vi.fn(async () => ({ source_message_id: 'om_other', image_key: 'img_x', image: { ref: 'saved' } })),
}))
const commands = new Commands(() => testDeployment(), {}, new AbortController().signal)
const event: FeishuRef = { triggerId: 'source', chatId: 'oc_chat', messageId: 'om_source' }
const source = { message_id: 'om_source', chat_id: 'oc_chat', parent_id: 'om_parent', root_id: 'om_root', thread_id: 'omt_thread' }
const message = { message_id: 'om_root', chat_id: 'oc_chat', msg_type: 'merge_forward', content: '<forwarded_messages>Earlier discussion ![Image](img_one)</forwarded_messages>', sender: { id: 'ou_user', name: 'User', sender_type: 'user' }, create_time: '2026-09-11 12:00' }
afterEach(() => vi.restoreAllMocks())
it('resolves legacy source metadata and reads merged forwards without embedding automatic thread expansion', async () => {
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ messages: [{ ...message, thread_replies: [{ content: 'Automatic expansion omitted' }] }] })
  const result = await readFeishu(commands, event, 'feishu/source/root', 20)
  expect(result).toMatchObject({ status: 'available', message: { content: message.content } })
  // The read response states no limitations of its own; the skill states them once.
  expect(result).not.toHaveProperty('limitations')
  expect(JSON.stringify(result)).not.toContain('Automatic expansion omitted')
  expect(lark.mock.calls[1]![0]).toEqual(['im', '+messages-mget', '--message-ids', 'om_root', '--no-reactions'])
})
it('paginates source thread messages with opaque cursors and retains authors and edited timestamps', async () => {
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: true, page_token: 'a/b+next=', messages: [{ ...message, message_id: 'om_reply', thread_id: 'omt_thread', updated: true, update_time: '2026-09-11 12:01' }] })
  const first = await readFeishu(commands, event, 'feishu/source/thread', 2)
  expect(first).toMatchObject({ next_resource: 'feishu/source/thread/a%2Fb%2Bnext%3D', messages: [{ sender: message.sender, updated: true }] })
  lark.mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: false, messages: [] })
  await readFeishu(commands, event, 'feishu/source/thread/a%2Fb%2Bnext%3D', 2)
  expect(lark.mock.lastCall![0]).toContain('a/b+next=')
})
it('skips and counts a topic message it cannot read, and reads the others', async () => {
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: false,
    messages: [{ ...message, message_id: 'om_reply', thread_id: 'omt_thread' }, { ...message, message_id: 'om_odd',
      sender: { sender_type: 'app' } }] })
  const page = await readFeishu(commands, event, 'feishu/source/thread', 20)
  expect(page).toMatchObject({ skipped: 1, messages: [{ message_id: 'om_reply' }] })
})
it('rejects unrelated paths, missing sources, cross-chat results, and incomplete pagination', async () => {
  const lark = vi.spyOn(commands, 'lark')
  await expect(readFeishu(commands, event, 'feishu/other/thread', 20)).rejects.toThrow('source trigger')
  await expect(readFeishu(commands, event, 'feishu/source/root/cursor', 20)).rejects.toThrow('Use feishu')
  expect(lark).not.toHaveBeenCalled()
  lark.mockResolvedValueOnce({ items: [] })
  await expect(readFeishu(commands, event, 'feishu/source/message', 20)).rejects.toThrow('unavailable')
  lark.mockResolvedValueOnce({ items: [{ ...source, chat_id: 'oc_elsewhere' }] })
  await expect(readFeishu(commands, event, 'feishu/source/root', 20)).rejects.toThrow('chat does not match')
  lark.mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: true, messages: [] })
  await expect(readFeishu(commands, event, 'feishu/source/thread', 20)).rejects.toThrow('did not advance')
})
it('reports absent parent/thread and deleted content explicitly', async () => {
  const lark = vi.spyOn(commands, 'lark').mockResolvedValue({ items: [{ message_id: 'om_source', chat_id: 'oc_chat' }] })
  expect(await readFeishu(commands, event, 'feishu/source/thread', 20)).toMatchObject({ status: 'no_thread', next_resource: null })
  expect(await readFeishu(commands, event, 'feishu/source/parent', 20)).toMatchObject({ status: 'no_parent', message: null })
  lark.mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ messages: [{ ...message, deleted: true }] })
  expect(await readFeishu(commands, event, 'feishu/source/root', 20)).toMatchObject({ status: 'deleted' })
})

it('reads only the source message link and rejects absent, deleted or cross-chat results', async () => {
  const linked = { message_id: 'om_source', chat_id: 'oc_chat', message_app_link: 'https://applink.feishu.cn/client/message/link/open?token=source' }
  const lark = vi.spyOn(commands, 'lark').mockResolvedValue({ messages: [{ ...linked, message_id: 'om_other', message_app_link: 'https://example.com/other' }, linked] })
  await expect(readFeishuLink(commands, event)).resolves.toBe(linked.message_app_link)
  expect(lark).toHaveBeenCalledWith(['im', '+messages-mget', '--message-ids', 'om_source', '--no-reactions'])
  for (const row of [{ ...linked, deleted: true }, { ...linked, chat_id: 'oc_elsewhere' }, { ...linked, message_app_link: undefined }, { ...linked, message_id: 'om_other' }]) {
    lark.mockResolvedValueOnce({ messages: [row] })
    await expect(readFeishuLink(commands, event)).rejects.toThrow('unavailable')
  }
})
it('reads a topic page as messages a model reads: id, author, ISO time, text and the message each answers', async () => {
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: false,
    messages: [{ ...message, message_id: 'om_reply', thread_id: 'omt_thread', parent_id: 'om_root', message_app_link: 'https://applink.feishu.cn/x' },
      { ...message, message_id: 'om_bot', sender: { id: 'cli_bot', name: 'Bot', sender_type: 'app' } }] })
  expect(await feishuRead(commands, event, 'feishu/source/thread', 20)).toEqual({ status: 'available', thread_id: 'omt_thread', next_resource: null,
    messages: [{ id: 'om_reply', from: 'User', at: new Date('2026-09-11 12:00').toISOString(), text: message.content, reply_to: 'om_root',
      url: 'https://applink.feishu.cn/x' }, { id: 'om_bot', from: 'Bot', at: new Date('2026-09-11 12:00').toISOString(), text: message.content, bot: true }] })
})
it('reads the chat around a message by time: five before it, the message and five after, then earlier pages', async () => {
  const at = Date.parse('2026-09-11T12:00:00Z')
  const item = (n: number) => ({ message_id: `om_${n}`, chat_id: 'oc_chat', create_time: String(at + n * 1000), sender: { id: 'ou_user', sender_type: 'user' },
    body: { content: JSON.stringify({ text: `message ${n}` }) } })
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] })
    .mockResolvedValueOnce({ items: [0, -1, -2, -3, -4, -5].map(item), has_more: true, page_token: 'older' })
    .mockResolvedValueOnce({ items: [0, 1, 2].map(item), has_more: false })
  const ref = { ...event, at: new Date(at).toISOString() }
  const around = await feishuRead(commands, ref, 'feishu/source/around', 20)
  expect(around).toMatchObject({ status: 'available', next_resource: 'feishu/source/around/older' })
  expect('messages' in around && around.messages.map(message => [message.id, message.text]))
    .toEqual([-5, -4, -3, -2, -1, 0, 1, 2].map(n => [`om_${n}`, `message ${n}`]))
  const params = (call: number) => JSON.parse(lark.mock.calls[call]![0].at(-1)!) as Record<string, unknown>
  expect(params(1)).toMatchObject({ container_id: 'oc_chat', sort_type: 'ByCreateTimeDesc', end_time: String(at / 1000), page_size: 6 })
  expect(params(2)).toMatchObject({ sort_type: 'ByCreateTimeAsc', start_time: String(at / 1000), page_size: 6 })
  lark.mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ items: [-6, -7].map(item), has_more: false })
  const earlier = await feishuRead(commands, ref, 'feishu/source/around/older', 20)
  expect('messages' in earlier && earlier.messages.map(message => message.id)).toEqual(['om_-7', 'om_-6'])
  expect(params(4)).toMatchObject({ page_token: 'older', page_size: 10 })
})


it('exposes the resources and context of one accepted Feishu trigger', async () => {
  expect(feishuReferences(event)).toEqual({
    skill: 'dsh-superagent-feishu',
    message: 'feishu/source/message',
    root: 'feishu/source/root',
    parent: 'feishu/source/parent',
    thread: 'feishu/source/thread',
    around: 'feishu/source/around',
  })
  const trigger = feishuTrigger({
    message_id: 'om_source', chat_id: 'oc_chat', chat_type: 'group', sender_id: 'ou_user', sender_type: 'user',
    content: 'please look', create_time: '1757572800000', mentions: [{ id: 'ou_bot' }],
  }, 'ou_bot', 'cli_bot')!
  expect(feishuThreadContext.messageId(trigger)).toBe('om_source')
  const reply = { ...message, message_id: 'om_reply', thread_id: 'omt_thread', parent_id: 'om_source' }
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] })
    .mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: true, page_token: 'next', messages: [reply] })
  const page = await feishuThreadContext.read(commands, trigger, 'thread', undefined, { readPageSize: 2 })
  expect(page).toMatchObject({ status: 'available', next: 'next', messages: [{ id: 'om_reply', reply_to: 'om_source' }] })
  expect(lark.mock.lastCall![0]).toEqual(expect.arrayContaining(['im', '+threads-messages-list', '--thread', 'omt_thread']))
  lark.mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ thread_id: 'omt_thread', has_more: false, messages: [] })
  await feishuThreadContext.read(commands, trigger, 'thread', 'next', { readPageSize: 2 })
  expect(lark.mock.lastCall![0]).toContain('--page-token')
})

it('falls back to earlier chat messages when an accepted Feishu trigger has no topic thread', async () => {
  const trigger = feishuTrigger({
    message_id: 'om_source', chat_id: 'oc_chat', chat_type: 'group', sender_id: 'ou_user', sender_type: 'user',
    content: 'please look', create_time: '1757572800000', mentions: [{ id: 'ou_bot' }],
  }, 'ou_bot', 'cli_bot')!
  const history = { message_id: 'om_earlier', chat_id: 'oc_chat', create_time: '1757572700000',
    sender: { id: 'ou_user', sender_type: 'user' }, body: { content: JSON.stringify({ text: 'earlier' }) } }
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [{ message_id: 'om_source', chat_id: 'oc_chat' }] })
    .mockResolvedValueOnce({ items: [history], has_more: false })
    .mockResolvedValueOnce({ items: [], has_more: false })
  const neighbours = await feishuThreadContext.read(commands, trigger, 'thread', undefined, { readPageSize: 2 })
  expect(neighbours).toMatchObject({ status: 'chat_neighbours', messages: [{ id: 'om_earlier', text: 'earlier' }] })
})

it('reads one Feishu parent from its trigger context and rejects a trigger without source metadata', async () => {
  const trigger = feishuTrigger({
    message_id: 'om_source', chat_id: 'oc_chat', chat_type: 'group', sender_id: 'ou_user', sender_type: 'user',
    content: 'please look', create_time: '1757572800000', mentions: [{ id: 'ou_bot' }], parent_id: 'om_parent',
  }, 'ou_bot', 'cli_bot')!
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] })
    .mockResolvedValueOnce({ messages: [{ ...message, message_id: 'om_parent' }] })
  await expect(feishuThreadContext.read(commands, trigger, 'parent', undefined, { readPageSize: 2 }))
    .resolves.toMatchObject({ status: 'available', messages: [{ id: 'om_parent' }] })
  await expect(feishuThreadContext.read(commands, { ...trigger, source: 'github' }, 'parent', undefined, { readPageSize: 2 }))
    .rejects.toThrow('A Feishu source trigger is required')
})

it('delegates image keys and image reads to the Feishu admission reader', async () => {
  const trigger = feishuTrigger({
    message_id: 'om_source', chat_id: 'oc_chat', chat_type: 'group', sender_id: 'ou_user', sender_type: 'user',
    content: 'please look', create_time: '1757572800000', mentions: [{ id: 'ou_bot' }],
  }, 'ou_bot', 'cli_bot')!
  expect(feishuThreadContext.images!('look at img_x and img_x')).toEqual(['img_x'])
  await expect(feishuThreadContext.image!({} as Agent, commands, trigger,
    { id: 'om_other', text: 'look at img_x' }, 'img_x', '/tmp', new AbortController().signal))
    .resolves.toMatchObject({ source_message_id: 'om_other', image_key: 'img_x' })
  expect(readAdmissionImage).toHaveBeenCalledWith({} as Agent, commands,
    { triggerId: trigger.id, chatId: 'oc_chat', messageId: 'om_other', at: trigger.at }, 'look at img_x', 'img_x', '/tmp',
    expect.any(AbortSignal))
})

it('keeps unreadable or mismatched Feishu responses out of the model context', async () => {
  const thread = (result: unknown) => vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce(result)
  await expect(readFeishu(commands, event, 'feishu/source/thread', 20, thread({ thread_id: 'omt_other', has_more: false, messages: [] })))
    .rejects.toThrow('outside the source thread')
  await expect(readFeishu(commands, event, 'feishu/source/thread', 20, thread({ thread_id: 'omt_thread', has_more: false,
    messages: [{ ...message, message_id: 'om_elsewhere', chat_id: 'oc_elsewhere', thread_id: 'omt_thread' }] })))
    .rejects.toThrow('outside the source thread')
  await expect(readFeishu(commands, event, 'feishu/source/thread', 20, thread({ thread_id: 'omt_thread', has_more: true,
    page_token: 'next', messages: [{ ...message, message_id: 'om_reply', thread_id: 'omt_other' }] })))
    .rejects.toThrow('outside the source thread')
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ messages: [] })
  await expect(readFeishu(commands, event, 'feishu/source/message', 20)).rejects.toThrow('requested Feishu message is unavailable')
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ messages: [{ ...message, chat_id: 'oc_elsewhere' }] })
  await expect(readFeishu(commands, event, 'feishu/source/root', 20)).rejects.toThrow('outside the source chat')
})

it('formats absent names, unreadable times, and deleted Feishu messages without inventing content', async () => {
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] }).mockResolvedValueOnce({ messages: [{
    ...message, message_id: 'om_root', sender: { id: 'ou_user', sender_type: 'user' }, create_time: 'not-a-time', deleted: true,
    content: 'should not be shown', parent_id: undefined, message_app_link: undefined }] })
    .mockResolvedValue({ items: [source] })
  expect(await feishuRead(commands, event, 'feishu/source/root', 20)).toEqual({
    status: 'deleted', message: { id: 'om_root', from: 'ou_user', at: 'not-a-time', text: '', deleted: true },
  })
  const around = { ...event, at: 'not-a-time' }
  await expect(feishuRead(commands, around, 'feishu/source/around', 20)).rejects.toThrow('no time to read the chat around')
  await expect(feishuRead(commands, event, 'feishu/other/around', 20)).rejects.toThrow('source trigger')
  lark.mockResolvedValueOnce({ items: [{ ...source, parent_id: undefined }] })
  expect(await feishuRead(commands, event, 'feishu/source/parent', 20)).toEqual({ status: 'no_parent', message: null })
})


it('returns empty Feishu text when a live message has no content, and refuses direct around reads', async () => {
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] })
    .mockResolvedValueOnce({ messages: [{ ...message, content: undefined }] })
  expect(await feishuRead(commands, event, 'feishu/source/root', 20)).toMatchObject({ message: { text: '' } })
  await expect(readFeishu(commands, event, 'feishu/source/around', 20)).rejects.toThrow('read with feishuRead')
})

it('marks bot, deleted, replied, linked, and untimed history messages without inventing content', async () => {
  const at = Date.parse('2026-09-11T12:00:00Z')
  const history = { message_id: 'om_history', chat_id: 'oc_chat', create_time: 'not-a-time', deleted: true,
    parent_id: 'om_parent', message_app_link: 'https://applink.feishu.cn/history',
    sender: { id: 'ou_app', sender_type: 'app' }, body: { content: JSON.stringify({ text: 'hidden' }) } }
  const lark = vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [source] })
    .mockResolvedValueOnce({ items: [history], has_more: false })
    .mockResolvedValueOnce({ items: [], has_more: false })
  expect(await feishuRead(commands, { ...event, at: new Date(at).toISOString() }, 'feishu/source/around', 20))
    .toMatchObject({ messages: [{ id: 'om_history', from: 'ou_app', at: 'not-a-time', text: '', bot: true, deleted: true,
      reply_to: 'om_parent', url: 'https://applink.feishu.cn/history' }] })
  const withoutTime: FeishuRef = { ...event }
  delete withoutTime.at
  lark.mockResolvedValueOnce({ items: [source] })
  await expect(feishuRead(commands, withoutTime, 'feishu/source/around', 20)).rejects.toThrow('no time to read the chat around')
})

it('returns no message from a Feishu trigger context when the parent is absent', async () => {
  const trigger = feishuTrigger({
    message_id: 'om_source', chat_id: 'oc_chat', chat_type: 'group', sender_id: 'ou_user', sender_type: 'user',
    content: 'please look', create_time: '1757572800000', mentions: [{ id: 'ou_bot' }],
  }, 'ou_bot', 'cli_bot')!
  vi.spyOn(commands, 'lark').mockResolvedValueOnce({ items: [{ ...source, parent_id: undefined }] })
  await expect(feishuThreadContext.read(commands, trigger, 'parent', undefined, { readPageSize: 2 }))
    .resolves.toMatchObject({ status: 'no_parent', messages: [] })
})
