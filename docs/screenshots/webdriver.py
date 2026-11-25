"""A minimal W3C WebDriver client (standard library only) for tauri-driver."""
import base64
import json
import time
import urllib.error
import urllib.request

ELEMENT = "element-6066-11e4-a52e-4f735466cecc"


class Session:
    def __init__(self, url: str, application: str):
        self.url = url
        caps = {"capabilities": {"alwaysMatch": {"tauri:options": {"application": application}}}}
        self.id = self._call("POST", "/session", caps)["sessionId"]

    def _call(self, method: str, path: str, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(self.url + path, data=data, method=method,
                                     headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return json.loads(r.read() or b"{}").get("value", {})
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"{method} {path}: {e.read().decode(errors='replace')[:400]}") from None

    def cmd(self, method: str, path: str, body=None):
        return self._call(method, f"/session/{self.id}{path}", body)

    def js(self, script: str, *args):
        """Runs `script` (a function body; `arguments` holds args) in the page."""
        return self.cmd("POST", "/execute/sync", {"script": script, "args": list(args)})

    def wait(self, script: str, timeout: float = 20.0):
        """Polls until `script` returns a truthy value."""
        end = time.time() + timeout
        while time.time() < end:
            v = self.js(script)
            if v:
                return v
            time.sleep(0.2)
        raise TimeoutError(script)

    def screenshot(self, path: str):
        with open(path, "wb") as f:
            f.write(base64.b64decode(self.cmd("GET", "/screenshot")))

    def quit(self):
        try:
            self._call("DELETE", f"/session/{self.id}")
        except Exception:
            pass
