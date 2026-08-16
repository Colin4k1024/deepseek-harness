/**
 * dsh — Node.js SDK for DeepSeek Harness.
 *
 * Connects to a running dsh-web server and provides a simple interface
 * for agent chat interactions.
 *
 * @example
 * ```js
 * import { DshClient } from '@deepseek-ai/dsh-sdk';
 *
 * const client = new DshClient('http://localhost:3000');
 *
 * // Non-streaming
 * const response = await client.chat('What is 2+2?');
 * console.log(response.response);
 *
 * // Streaming
 * for await (const chunk of client.chatStream('Tell me a story')) {
 *   process.stdout.write(chunk);
 * }
 * ```
 */

export class DshError extends Error {
  constructor(message, status, body) {
    super(message);
    this.name = 'DshError';
    this.status = status;
    this.body = body;
  }
}

export class DshClient {
  /**
   * Create a new dsh client.
   *
   * @param {string} baseUrl - URL of the dsh-web server
   * @param {object} [options] - Client options
   * @param {string} [options.model] - Default LLM model name
   */
  constructor(baseUrl = 'http://localhost:3000', options = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.model = options.model || 'deepseek-chat';
  }

  /**
   * Check server health.
   * @returns {Promise<{status: string, version: string}>}
   */
  async health() {
    const resp = await fetch(`${this.baseUrl}/health`);
    return resp.json();
  }

  /**
   * Send a non-streaming chat request.
   *
   * @param {string} message - The user message
   * @param {object} [options] - Request options
   * @param {string} [options.model] - Override model name
   * @returns {Promise<{response: string, sessionEvents: number}>}
   */
  async chat(message, options = {}) {
    const resp = await fetch(`${this.baseUrl}/api/v1/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        message,
        model: options.model || this.model,
      }),
    });

    if (!resp.ok) {
      const body = await resp.text();
      throw new DshError(`HTTP ${resp.status}: ${body}`, resp.status, body);
    }

    const data = await resp.json();
    return {
      response: data.response,
      sessionEvents: data.session_events,
    };
  }

  /**
   * Send a streaming chat request (SSE).
   *
   * @param {string} message - The user message
   * @param {object} [options] - Request options
   * @param {string} [options.model] - Override model name
   * @yields {{event: string, data: string}} SSE events
   */
  async *chatStream(message, options = {}) {
    const resp = await fetch(`${this.baseUrl}/api/v1/chat/stream`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        message,
        model: options.model || this.model,
      }),
    });

    if (!resp.ok) {
      const body = await resp.text();
      throw new DshError(`HTTP ${resp.status}: ${body}`, resp.status, body);
    }

    const reader = resp.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });

      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith(':')) continue;

        if (trimmed.startsWith('data: ')) {
          try {
            const parsed = JSON.parse(trimmed.slice(6));
            yield {
              event: parsed.event || 'unknown',
              data: parsed.data || '',
            };
          } catch {
            // Skip malformed JSON
          }
        }
      }
    }
  }

  /**
   * Convenience: streaming chat that yields only text chunks.
   *
   * @param {string} message - The user message
   * @param {object} [options] - Request options
   * @yields {string} Text chunks as they arrive
   */
  async *chatStreamText(message, options = {}) {
    for await (const event of this.chatStream(message, options)) {
      if (event.event === 'chunk') {
        yield event.data;
      } else if (event.event === 'error') {
        throw new DshError(`Stream error: ${event.data}`);
      } else if (event.event === 'done') {
        break;
      }
    }
  }
}
