/** Standard ACP updates derived from live and committed DSH session events. */

import type { Context } from '@deepseek-ai/cordis'
import type { SessionUpdate, ToolCallContent } from '@agentclientprotocol/sdk'
import type { Session, SessionEvent } from '@deepseek-ai/dsh-session'
import type { StreamChunk } from '@deepseek-ai/dsh-llm'
import type {} from '@deepseek-ai/dsh-token-meter'
import { assistantBlockToAcp } from './content.ts'

/** Text already delivered to the ACP client for one model step. */
export type StreamedBlocks = Map<number, { type: 'text' | 'reasoning'; text: string }>

/** Project one live model delta without waiting for the committed message. */
export function assistantChunkUpdate(
  chunk: StreamChunk,
  messageId: string,
): SessionUpdate | undefined {
  if ((chunk.type !== 'text-delta' && chunk.type !== 'reasoning-delta') || chunk.text.length === 0) return undefined
  return {
    sessionUpdate: chunk.type === 'text-delta' ? 'agent_message_chunk' : 'agent_thought_chunk',
    messageId,
    content: { type: 'text', text: chunk.text },
  }
}

/**
 * Convert one committed assistant message and its context usage in block order.
 * @param ctx - bridge context carrying attachment and token-meter services.
 * @param session - durable session used for context pressure.
 * @param event - committed assistant message event.
 * @param streamed - text already delivered from this step's durable chunks.
 * @param liveMessageId - stable ACP identity shared with those live chunks.
 * @returns ordered standard thought, message, and optional usage updates.
 */
export async function assistantUpdates(
  ctx: Context,
  session: Session,
  event: SessionEvent<'assistant/message'>,
  streamed?: StreamedBlocks,
  liveMessageId?: string,
): Promise<SessionUpdate[]> {
  const updates: SessionUpdate[] = []
  const streamedOrder = streamed?.values()
  for (const block of event.data.message.content) {
    const delivered = block.type === 'text' || block.type === 'reasoning'
      ? streamedOrder?.next().value
      : undefined
    const text = block.type === 'text' || block.type === 'reasoning' ? block.text : undefined
    // A committed block may extend its live prefix (for example after interruption).
    // If it differs, publish the authoritative block with its durable identity.
    if (text !== undefined && delivered !== undefined && delivered.type === block.type && text.startsWith(delivered.text)) {
      const suffix = text.slice(delivered.text.length)
      if (suffix.length > 0) updates.push({
        sessionUpdate: block.type === 'reasoning' ? 'agent_thought_chunk' : 'agent_message_chunk',
        messageId: liveMessageId ?? event.data.message.id,
        content: { type: 'text', text: suffix },
      })
      continue
    }
    if (block.type === 'reasoning') {
      if (block.text.length > 0) {
        updates.push({
          sessionUpdate: 'agent_thought_chunk',
          messageId: liveMessageId ?? event.data.message.id,
          content: { type: 'text', text: block.text },
        })
      }
      continue
    }
    const content = await assistantBlockToAcp(ctx, block)
    if (content !== undefined) {
      updates.push({
        sessionUpdate: 'agent_message_chunk',
        messageId: liveMessageId ?? event.data.message.id,
        content,
      })
    }
  }
  const usage = usageUpdate(ctx, session, event)
  if (usage !== undefined) updates.push(usage)
  return updates
}

/**
 * Start one generic ACP tool lifecycle from the durable call fact.
 * @param event - committed DSH tool-call event.
 * @returns the standard generic tool-call update.
 */
export function toolCallUpdate(event: SessionEvent<'tool/call'>): SessionUpdate {
  return {
    sessionUpdate: 'tool_call',
    toolCallId: event.data.callId,
    title: event.data.name,
    kind: 'other',
    status: 'in_progress',
    rawInput: parseToolArguments(event.data.arguments),
  }
}

/**
 * Finish one generic ACP tool lifecycle from its committed model-facing result.
 * @param ctx - bridge context carrying the attachment store.
 * @param event - committed DSH tool-result event.
 * @returns the standard completed or failed tool-call update.
 */
export async function toolResultUpdate(
  ctx: Context,
  event: SessionEvent<'tool/result'>,
): Promise<SessionUpdate> {
  const message = event.data.message
  const content: ToolCallContent[] = []
  for (const block of message.content) {
    const converted = await assistantBlockToAcp(ctx, block)
    if (converted !== undefined) content.push({ type: 'content' as const, content: converted })
  }
  return {
    sessionUpdate: 'tool_call_update',
    toolCallId: message.toolCallId,
    status: message.isError === true ? 'failed' : 'completed',
    content,
  }
}

/** Report current context occupancy only when DSH has both usage and capacity facts. */
function usageUpdate(
  ctx: Context,
  session: Session,
  event: SessionEvent<'assistant/message'>,
): SessionUpdate | undefined {
  if (event.data.usage === undefined) return undefined
  const size = session.requestContext()?.contextWindow
  const meter = ctx.get('tokenMeter')
  if (size === undefined || meter === undefined) return undefined
  return {
    sessionUpdate: 'usage_update',
    used: meter.measure(session).totalTokens,
    size,
  }
}

/** Preserve malformed model output as opaque input instead of dropping the call update. */
function parseToolArguments(value: string): unknown {
  try {
    return JSON.parse(value) as unknown
  } catch (_invalidModelJson) {
    return value
  }
}
