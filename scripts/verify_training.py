"""Exercise all training lessons through HTTP on a disposable local server.

Usage: python scripts/verify_training.py --url http://127.0.0.1:8792 --allow-reset
This deliberately resets the server's battle and writes its training results.
Only public Blue observations, normal commands and training actions are used.
"""
import argparse
import json
import time
import urllib.request
import urllib.error
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--allow-reset", action="store_true")
    args = parser.parse_args()
    if not args.allow_reset:
        parser.error("--allow-reset is required: this test replaces the active battle")
    base = args.url.rstrip("/")

    def request(path, body=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(base + path, data=data, headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(req, timeout=10) as response:
            return json.load(response)

    def status():
        return request("/api/tutorial/status")["session"]

    def action(name, session, **extra):
        return request("/api/tutorial/action", {"action": name, "attempt": session["attempt"], "step": session["step"], **extra})["session"]

    def command(value):
        return request("/api/command", {"side": "Blue", "command": value})

    def rejected(call):
        try:
            call()
        except urllib.error.HTTPError as error:
            assert error.code == 409, error.code
        else:
            raise AssertionError("Invalid action was accepted")

    fixture = json.loads((Path(__file__).resolve().parents[1] / "data/training/courses.json").read_text(encoding="utf-8"))
    public = request("/api/tutorial/catalog")
    assert '"answer":' not in json.dumps(public), "Answer keys leaked through public catalogue"
    try:
        request("/training/courses.json")
    except urllib.error.HTTPError as error:
        assert error.code == 404
    else:
        raise AssertionError("Private curriculum is exposed in the static web root")
    probe = request("/api/tutorial/start", {"course": "navigation"})["session"]
    previous = probe
    probe = action("ack", probe)
    rejected(lambda: action("ack", previous))
    probe = action("select_unit", probe, unit=probe["unit"])
    rejected(lambda: action("ack", probe))
    command({"cmd": "set_speed", "unit": probe["unit"], "speed_kts": 4})
    probe = action("step", probe)
    assert probe["step"] == 2, "Wrong speed passed a practical objective"
    probe = action("retry", probe)
    restored = request("/api/state?side=Blue")
    assert restored["tick"] == probe["checkpoint_tick"]
    assert next(u for u in restored["units"] if u["id"] == probe["unit"])["speed_kts"] == 0
    assert request("/api/time")["speed"] == 0
    print("PASS stale actions, non-skippable objectives, wrong speed, checkpoint restore and private curriculum", flush=True)
    results = []
    for course in fixture["courses"]:
        session = request("/api/tutorial/start", {"course": course["id"]})["session"]
        deadline = time.monotonic() + 45
        while session["status"] == "active":
            assert time.monotonic() < deadline, (course["id"], "deadline", session)
            index = session["step"]
            step = course["steps"][index]
            kind = step["objective"]
            unit = session["unit"]
            state = request("/api/state?side=Blue")
            own = next(u for u in state["units"] if u["id"] == unit)
            if kind in ("ack", "report"):
                session = action("ack", session)
            elif kind == "select_unit":
                session = action("select_unit", session, unit=unit)
            elif kind == "select_contact":
                contact = next(c for c in state["contacts"] if not c["stale"])
                session = action("select_contact", session, contact=contact["id"])
            elif kind == "question":
                wrong = next(o["id"] for o in step["options"] if o["id"] != step["answer"])
                check = action("answer", session, answer=wrong)
                assert check["step"] == index and check["feedback"] == "incorrect_answer"
                session = action("answer", check, answer=step["answer"])
            elif kind in ("running", "paused"):
                request("/api/time", {"speed": 15 if kind == "running" else 0})
                time.sleep(0.12)
                session = status()
            else:
                value = None
                if kind == "speed": value = {"cmd": "set_speed", "speed_kts": step["target"]}
                if kind == "course": value = {"cmd": "set_course", "course_deg": step["target"]}
                if kind == "depth": value = {"cmd": "set_depth", "depth_m": step["target"]}
                if kind in ("sonar_on", "sonar_off"): value = {"cmd": "set_sonar", "active": kind == "sonar_on"}
                if kind == "route":
                    command({"cmd": "set_speed", "unit": unit, "speed_kts": 12})
                    value = {"cmd": "goto", "waypoints": [{"x": 1, "y": 0}]}
                if kind == "stop": value = {"cmd": "stop"}
                if kind == "gun_fire":
                    target = next(c for c in state["contacts"] if c["category"] == "Ship" and not c["stale"])
                    value = {"cmd": "fire", "contact": target["id"], "weapon": "gun_5in", "rounds": 1}
                if kind == "chaff": value = {"cmd": "deploy_chaff"}
                if kind == "launch": value = {"cmd": "launch_aircraft", "class": "helo_asw", "count": 1, "mission": {"mission": "asw", "area": own["pos"], "radius_nm": 12}}
                if kind == "recall": value = {"cmd": "recover_aircraft"}
                if value:
                    value["unit"] = unit
                    command(value)
                session = action("step", session)
                if session["step"] == index and kind in ("arrive", "depth"):
                    request("/api/time", {"speed": 300})
                    for _ in range(100):
                        time.sleep(0.03)
                        session = status()
                        if session["step"] != index or session["status"] != "active": break
                elif session["step"] == index and kind in ("contact", "identified_contact", "incoming", "recovered"):
                    for _ in range(500):
                        session = action("step", session)
                        if session["step"] != index or session["status"] != "active": break
                assert session["step"] != index or session["status"] != "active", (course["id"], step["id"], session)
            assert session["status"] != "failed", (course["id"], step["id"], session)
        assert session["status"] == "completed"
        assert request("/api/tutorial/status")["progress_saved"], "Completion record was not persisted"
        rejected(lambda: action("step", session))
        results.append({"course": course["id"], "objectives": session["step_count"], "status": session["status"]})
        print("PASS", course["id"], session["step_count"], "objectives", flush=True)
    assert len(request("/api/tutorial/status")["completed"]) == len(fixture["courses"])
    print(json.dumps({"courses": len(results), "objectives": sum(r["objectives"] for r in results), "result": "PASS", "details": results}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
