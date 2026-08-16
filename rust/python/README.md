# dsh — Python SDK for DeepSeek Harness

## Installation

```bash
pip install -e ./python
```

## Usage

```python
from dsh import DshClient

client = DshClient("http://localhost:3000")

# Non-streaming
response = client.chat("What is 2+2?")
print(response.response)  # "4"
print(response.session_events)  # 3

# Streaming
for chunk in client.chat_stream_text("Tell me a story"):
    print(chunk, end="", flush=True)
```

## API

### `DshClient(base_url, api_key=None, model="deepseek-chat")`

Create a client connected to a dsh-web server.

### `client.health()`

Check server health. Returns `{"status": "ok", "version": "..."}`.

### `client.chat(message, model=None)`

Non-streaming chat. Returns `ChatResponse(response, session_events)`.

### `client.chat_stream(message, model=None)`

Streaming chat (SSE). Yields `StreamEvent(event, data)`.

### `client.chat_stream_text(message, model=None)`

Convenience streaming. Yields text chunks only.

## Requirements

- Python 3.8+
- No external dependencies (uses `urllib`)
