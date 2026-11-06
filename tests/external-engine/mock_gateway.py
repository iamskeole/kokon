#!/usr/bin/env python3
"""Mock llama-swap `/upstream/<model>` gateway for the kokon external-engine integration test.

Single-tenant like the real one: one DIT loaded at a time; a request for a different model
evicts the current one, then 'loads' the new one after LOAD_S seconds. Every request path
is logged so the test can prove all engine traffic used the /upstream/<model> prefix.

  usage: mock_gateway.py PORT LOAD_S REQUESTS_LOG
"""
import base64, json, sys, threading, time, uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

MODELS = {
    "kokon-qwen-image-2.1-turbo": "qwen-image-2.1-turbo-Q4_K_M.gguf",
    "kokon-qwen-image-2.1-full": "qwen-image-2.1-Q8_0.gguf",
}
PORT = int(sys.argv[1])
LOAD_S = float(sys.argv[2]) if len(sys.argv) > 2 else 1.5
LOG = sys.argv[3] if len(sys.argv) > 3 else None

state = {"loaded": None, "evicts": 0, "loads": 0, "jobs": {}}
lock = threading.Lock()

# 1x1 red PNG (generated)
PNG_1PX = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"


def log(path):
    if LOG:
        with open(LOG, "a") as f:
            f.write(json.dumps({"t": time.time(), "path": path}) + "\n")


class H(BaseHTTPRequestHandler):
    def _send(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        try:
            self.wfile.write(body)
        except BrokenPipeError:
            pass

    def _body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n)) if n else {}

    def _ensure(self, model):
        """llama-swap's on-demand load: evict the other tenant, then load (takes LOAD_S)."""
        with lock:
            if state["loaded"] != model:
                if state["loaded"] is not None:
                    state["evicts"] += 1
                state["loaded"] = None
                time.sleep(LOAD_S)
                state["loaded"] = model
                state["loads"] += 1

    def do_GET(self):
        log(self.path)
        p = [x for x in self.path.split("/") if x]
        if len(p) >= 5 and p[0] == "upstream" and p[2] == "sdcpp" and p[3] == "v1" and p[1] in MODELS:
            kind = p[4]
            if kind == "capabilities":
                self._ensure(p[1])
                return self._send(200, {
                    "model": {"name": MODELS[p[1]]},
                    "supported_modes": ["img_gen"],
                    "samplers": ["euler"],
                    "upscalers": [],
                    "features_by_mode": {"img_gen": {}},
                })
            if kind == "jobs" and len(p) == 6:
                j = state["jobs"].get(p[5])
                return self._send(200, j) if j else self._send(404, {"error": "no such job"})
        if p == ["_control", "state"]:
            with lock:
                return self._send(200, state)
        return self._send(404, {"error": "not found: " + self.path})

    def do_POST(self):
        log(self.path)
        p = [x for x in self.path.split("/") if x]
        if len(p) == 5 and p[0] == "upstream" and p[2] == "sdcpp" and p[3] == "v1" and p[4] == "img_gen" and p[1] in MODELS:
            self._body()
            self._ensure(p[1])
            jid = uuid.uuid4().hex
            with lock:
                state["jobs"][jid] = {"id": jid, "status": "queued"}

            def finish():
                time.sleep(0.4)
                with lock:
                    state["jobs"][jid] = {
                        "id": jid,
                        "status": "completed",
                        "result": {"images": [{"index": 0, "b64_json": PNG_1PX}], "output_format": "png"},
                    }

            threading.Thread(target=finish, daemon=True).start()
            return self._send(202, {"id": jid, "kind": "img_gen", "status": "queued"})
        if p == ["_control", "reset"]:
            with lock:
                state["loaded"] = None
                state["jobs"].clear()
            return self._send(200, {"ok": True})
        return self._send(404, {"error": "not found: " + self.path})

    def log_message(self, *a):
        pass


ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
