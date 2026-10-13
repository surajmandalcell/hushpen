#!/usr/bin/env python3
"""A small OpenAI-compatible server for the endpoint suite.

usage: openai-stub.py <log.jsonl> [port]

Listens on 127.0.0.1 (11434 by default, the Ollama port). Every request is appended to the log as
one JSON line: method, path, the Authorization header, and the parsed body. The reply to a chat
completion depends on the first line of /tmp/stub-mode, read on every request:

  default      the transcript from the <transcript> tags, with " [stub]" added; for a Command Mode
               request ("Instruction: ..." then "Text:" and the selection) the selection with
               " [edited]" added
  edited       "EDITED TEXT" for every request
  poem         a 400 character poem
  refuse       "I cannot help with that."
  slow         wait 30 s, then answer like default
  status:<n>   answer with that HTTP status and an error body
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LOG = sys.argv[1]
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 11434
MODE_FILE = "/tmp/stub-mode"
POEM = (
    "The morning light spills over hills of green, a quiet river sings its sleepy tune, "
    "and every leaf holds dew like silver sheen beneath the pale and slowly fading moon. "
    "The farmer wakes to bells of distant sheep, the baker dusts his hands with golden flour, "
    "while children in their beds still fast asleep dream of the swallows at the evening hour. "
    "So let the day begin with gentle, quiet, tender hands."
)


def mode():
    try:
        with open(MODE_FILE) as handle:
            return handle.readline().strip() or "default"
    except OSError:
        return "default"


def selection_of(body):
    """The selection of a Command Mode request, or None for any other request."""
    for message in reversed(body.get("messages", [])):
        content = message.get("content", "")
        if message.get("role") == "user" and isinstance(content, str):
            if content.startswith("Instruction: ") and "\n\nText:\n" in content:
                return content.split("\n\nText:\n", 1)[1]
            return None
    return None


def transcript_of(body):
    for message in reversed(body.get("messages", [])):
        content = message.get("content", "")
        if message.get("role") == "user" and isinstance(content, str):
            start, end = content.find("<transcript>"), content.find("</transcript>")
            if start >= 0 and end > start:
                return content[start + len("<transcript>"):end].strip()
            return content.strip()
    return ""


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def record(self, body):
        entry = {
            "t_ms": int(time.time() * 1000),
            "method": self.command,
            "path": self.path,
            "authorization": self.headers.get("Authorization"),
            "body": body,
        }
        with open(LOG, "a") as handle:
            handle.write(json.dumps(entry) + "\n")

    def send_json(self, status, payload):
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def handle_any(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            body = json.loads(raw) if raw else None
        except ValueError:
            body = {"unparsed": raw.decode("utf-8", "replace")}
        self.record(body)
        current = mode()
        if current.startswith("status:"):
            status = int(current.split(":", 1)[1])
            return self.send_json(status, {"error": {"message": "stub error"}})
        if current == "slow":
            time.sleep(30)
        path = self.path.split("?", 1)[0].rstrip("/")
        if self.command == "GET" and path.endswith("/models"):
            return self.send_json(
                200, {"object": "list", "data": [{"id": "stub-model", "object": "model"}]}
            )
        if self.command == "POST" and path.endswith("/chat/completions"):
            if current == "poem":
                text = POEM
            elif current == "refuse":
                text = "I cannot help with that."
            elif current == "edited":
                text = "EDITED TEXT"
            elif selection_of(body or {}) is not None:
                text = selection_of(body) + " [edited]"
            else:
                text = transcript_of(body or {}) + " [stub]"
            return self.send_json(
                200,
                {
                    "id": "stub-1",
                    "object": "chat.completion",
                    "model": (body or {}).get("model", "stub-model"),
                    "choices": [
                        {
                            "index": 0,
                            "message": {"role": "assistant", "content": text},
                            "finish_reason": "stop",
                        }
                    ],
                },
            )
        return self.send_json(404, {"error": {"message": "not found"}})

    do_GET = handle_any
    do_POST = handle_any


class Server(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    Server(("127.0.0.1", PORT), Handler).serve_forever()
