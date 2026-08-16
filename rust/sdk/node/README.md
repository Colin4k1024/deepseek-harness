# @deepseek-ai/dsh-sdk — Node.js SDK for DeepSeek Harness

## Installation

```bash
npm install ./sdk/node
```

## Usage

```js
import { DshClient } from '@deepseek-ai/dsh-sdk';

const client = new DshClient('http://localhost:3000');

// Non-streaming
const response = await client.chat('What is 2+2?');
console.log(response.response); // "4"

// Streaming
for await (const chunk of client.chatStreamText('Tell me a story')) {
  process.stdout.write(chunk);
}
```

## API

### `new DshClient(baseUrl, options?)`

Create a client. Options: `{ model: "deepseek-chat" }`.

### `client.health()`

Check server health.

### `client.chat(message, options?)`

Non-streaming chat. Returns `{ response, sessionEvents }`.

### `client.chatStream(message, options?)`

Streaming chat (SSE). Async generator yielding `{ event, data }`.

### `client.chatStreamText(message, options?)`

Convenience streaming. Async generator yielding text chunks.

## Requirements

- Node.js 18+ (uses native `fetch` and `ReadableStream`)
- No external dependencies
