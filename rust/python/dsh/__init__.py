"""dsh — Python SDK for DeepSeek Harness.

Connects to a running dsh-web server and provides a simple interface
for agent chat interactions.

Usage:
    from dsh import DshClient

    client = DshClient("http://localhost:3000")

    # Non-streaming
    response = client.chat("What is 2+2?")
    print(response.response)

    # Streaming
    for chunk in client.chat_stream("Tell me a story"):
        print(chunk, end="", flush=True)
"""

from .client import DshClient, ChatResponse, StreamEvent

__version__ = "0.1.0"
__all__ = ["DshClient", "ChatResponse", "StreamEvent"]
