"""dsh HTTP client — connects to dsh-web API."""

from dataclasses import dataclass
from typing import Iterator, Optional
import json
import urllib.request
import urllib.error


@dataclass
class ChatResponse:
    """Response from a non-streaming chat request."""
    response: str
    session_events: int


@dataclass
class StreamEvent:
    """A single SSE event from a streaming chat request."""
    event: str  # "chunk", "done", "error"
    data: str


class DshError(Exception):
    """Error from dsh API."""
    pass


class DshClient:
    """Client for the dsh-web HTTP API.

    Args:
        base_url: URL of the dsh-web server (default: http://localhost:3000)
        api_key: DeepSeek API key (optional, can also be set via DEEPSEEK_API_KEY)
        model: LLM model name (default: deepseek-chat)
    """

    def __init__(
        self,
        base_url: str = "http://localhost:3000",
        api_key: Optional[str] = None,
        model: str = "deepseek-chat",
    ):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.model = model

    def health(self) -> dict:
        """Check server health."""
        url = f"{self.base_url}/health"
        req = urllib.request.Request(url, method="GET")
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read())

    def chat(self, message: str, model: Optional[str] = None) -> ChatResponse:
        """Send a non-streaming chat request.

        Args:
            message: The user message
            model: Override model name (optional)

        Returns:
            ChatResponse with the agent's response and event count
        """
        url = f"{self.base_url}/api/v1/chat"
        body = json.dumps({
            "message": message,
            "model": model or self.model,
        }).encode()

        req = urllib.request.Request(
            url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(req) as resp:
                data = json.loads(resp.read())
                return ChatResponse(
                    response=data["response"],
                    session_events=data["session_events"],
                )
        except urllib.error.HTTPError as e:
            error_body = e.read().decode() if e.fp else str(e)
            raise DshError(f"HTTP {e.code}: {error_body}") from e

    def chat_stream(self, message: str, model: Optional[str] = None) -> Iterator[StreamEvent]:
        """Send a streaming chat request (SSE).

        Args:
            message: The user message
            model: Override model name (optional)

        Yields:
            StreamEvent for each SSE event
        """
        url = f"{self.base_url}/api/v1/chat/stream"
        body = json.dumps({
            "message": message,
            "model": model or self.model,
        }).encode()

        req = urllib.request.Request(
            url,
            data=body,
            headers={"Content-Type": "application/json"},
            method="POST",
        )

        try:
            with urllib.request.urlopen(req) as resp:
                buffer = ""
                while True:
                    chunk = resp.read(4096)
                    if not chunk:
                        break
                    buffer += chunk.decode()

                    while "\n" in buffer:
                        line, buffer = buffer.split("\n", 1)
                        line = line.strip()
                        if not line:
                            continue
                        if line.startswith("data: "):
                            data = line[6:]
                            try:
                                parsed = json.loads(data)
                                yield StreamEvent(
                                    event=parsed.get("event", "unknown"),
                                    data=parsed.get("data", ""),
                                )
                            except json.JSONDecodeError:
                                pass
        except urllib.error.HTTPError as e:
            error_body = e.read().decode() if e.fp else str(e)
            raise DshError(f"HTTP {e.code}: {error_body}") from e

    def chat_stream_text(self, message: str, model: Optional[str] = None) -> Iterator[str]:
        """Convenience: streaming chat that yields only text chunks.

        Args:
            message: The user message
            model: Override model name (optional)

        Yields:
            Text chunks as they arrive
        """
        for event in self.chat_stream(message, model):
            if event.event == "chunk":
                yield event.data
            elif event.event == "error":
                raise DshError(f"Stream error: {event.data}")
            elif event.event == "done":
                break
