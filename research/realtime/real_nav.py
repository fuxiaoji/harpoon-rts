"""Persistent JSON-lines adapter to exact frozen-core navigation segments.

Opaque frame refs belong to one process; use dump/load for cross-process replay.
Never send refs, judge worlds, or model-independent caches to the LLM.
"""
from __future__ import annotations
import copy
import json
from pathlib import Path
import subprocess
import time


class RealNav:
    def __init__(self, binary):
        self.binary = str(Path(binary).resolve())
        self.process = subprocess.Popen([self.binary], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            encoding="utf-8", bufsize=1)
        self.cache = {}
        self.wall_s = 0.0
        self.requests = 0

    def _request(self, data):
        start = time.perf_counter()
        self.process.stdin.write(json.dumps(data, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        self.wall_s += time.perf_counter() - start
        self.requests += 1
        if not line:
            raise RuntimeError("Navigation service closed: " + self.process.stderr.read()[-2000:])
        response = json.loads(line)
        if "error" in response:
            raise ValueError(response["error"])
        return response["ok"]

    def initial(self, pos, course=90):
        return self._request({"op": "initial", "pos": pos, "course": course})

    def segment(self, state, goal=None, seconds=2400, stop=False, until_arrival=True):
        if type(seconds) is not int or not 0 <= seconds <= 10000:
            raise ValueError("seconds must be integer 0..10000")
        q = {"op": "segment", "state": state, "goal": goal, "seconds": seconds,
             "stop": stop, "until_arrival": until_arrival}
        key = json.dumps(q, sort_keys=True, separators=(",", ":"))
        if key not in self.cache:
            self.cache[key] = self._request(q)
        return copy.deepcopy(self.cache[key])

    def advance(self, state, goal=None, seconds=2400, stop=False, until_arrival=True):
        return self.segment(state, goal, seconds, stop, until_arrival)[-1]

    def fingerprint(self, state):
        return self._request({"op": "fingerprint", "state": state})

    def dump(self, state):
        return self._request({"op": "dump", "state": state})

    def load(self, world):
        return self._request({"op": "load", "world": world})

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=5)

    def __enter__(self): return self
    def __exit__(self, *args): self.close()
