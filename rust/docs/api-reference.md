# API Reference — dsh-rust Web Server

## Base URL

```
http://localhost:3000
```

## Authentication

All API endpoints (except `/health`) require an API key passed via the `Authorization` header:

```
Authorization: Bearer sk-your-api-key
```

## Endpoints

### Health Check

```
GET /health
```

Returns server health status.

**Response** (200 OK):
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Chat (Non-Streaming)

```
POST /api/v1/chat
```

Send a message and receive a complete response.

**Request Body**:
```json
{
  "message": "What is 2+2?",
  "model": "deepseek-chat"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `message` | string | Yes | The user message |
| `model` | string | No | LLM model name (default: `deepseek-chat`) |

**Response** (200 OK):
```json
{
  "response": "4",
  "session_events": 3
}
```

| Field | Type | Description |
|-------|------|-------------|
| `response` | string | The agent's response |
| `session_events` | number | Number of events in the session log |

**Error Response** (500):
```json
{
  "error": "LLM provider error: rate limited"
}
```

---

### Chat (Streaming)

```
POST /api/v1/chat/stream
```

Send a message and receive a streaming response via Server-Sent Events (SSE).

**Request Body**: Same as non-streaming chat.

**Response** (200 OK, `text/event-stream`):

```
data: {"event":"chunk","data":"The answer is"}

data: {"event":"chunk","data":" 4."}

data: {"event":"done","data":"3 events"}
```

**Event Types**:

| Event | Description |
|-------|-------------|
| `chunk` | A text chunk of the response |
| `done` | Stream complete (data contains event count) |
| `error` | An error occurred (data contains error message) |

---

## Error Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 401 | Invalid or missing API key |
| 429 | Rate limit exceeded |
| 500 | Internal server error |

---

## Rate Limiting

When rate limited, the response includes headers:

```
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1692000000
```

---

## Examples

### curl (Non-Streaming)

```bash
curl -X POST http://localhost:3000/api/v1/chat \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-xxx" \
  -d '{"message": "What is 2+2?"}'
```

### curl (Streaming)

```bash
curl -X POST http://localhost:3000/api/v1/chat/stream \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk-xxx" \
  -d '{"message": "Tell me a story"}' \
  --no-buffer
```

### Python

```python
from dsh import DshClient

client = DshClient("http://localhost:3000")

# Non-streaming
response = client.chat("What is 2+2?")
print(response.response)

# Streaming
for chunk in client.chat_stream_text("Tell me a story"):
    print(chunk, end="", flush=True)
```

### Node.js

```js
import { DshClient } from '@deepseek-ai/dsh-sdk';

const client = new DshClient('http://localhost:3000');

// Non-streaming
const response = await client.chat('What is 2+2?');
console.log(response.response);

// Streaming
for await (const chunk of client.chatStreamText('Tell me a story')) {
  process.stdout.write(chunk);
}
```
