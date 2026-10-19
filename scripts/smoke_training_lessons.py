#!/usr/bin/env python3
"""Drive the three practical-academy courses end to end via the HTTP API.

Proves each lesson is completable: every objective is reached with real
engine evidence (no assertion is bypassed).

Usage: python3 scripts/smoke_training_lessons.py [base_url]
"""
import json
import sys
import urllib.request

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8787"


def req(path, payload=None):
    data = None if payload is None else json.dumps(payload).encode()
    r = urllib.request.Request(BASE + path, data=data,
                               headers={"content-type": "application/json"})
    try:
        with urllib.request.urlopen(r, timeout=30) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as e:
        raise SystemExit(f"HTTP {e.code} on {path}: {e.read().decode()[:400]}")


def status():
    return req("/api/tutorial/status")["session"]


def state():
    return req("/api/state?side=Blue&max_events=40")


def cmd(command):
    req("/api/command", {"side": "Blue", "command": command})


def advance(session, limit=2000, name=""):
    """Press 'step' (one simulated second) until the objective changes."""
    start = session["step"]
    for i in range(limit):
        s = status()
        if s is None:
            return None
        if s["step"] != start or s["status"] != "active":
            return s
        s = req("/api/tutorial/action",
                {"action": "step", "attempt": s["attempt"], "step": s["step"]})
    raise SystemExit(f"timeout: objective did not clear after {limit}s ({name})")


def act(session, action, **extra):
    body = {"action": action, "attempt": session["attempt"],
            "step": session["step"], **extra}
    return req("/api/tutorial/action", body)["session"]


def show(tag, s):
    print(f"  [{tag}] step {s['step']}/{s['step_count']} status={s['status']} feedback={s['feedback']}")


def run_course(course_id, script):
    print(f"\n=== {course_id} ===")
    view = req("/api/tutorial/start", {"course": course_id})
    s = view["session"]
    assert s and s["status"] == "active", view
    show("start", s)
    script(s)
    final = status()
    show("final", final)
    assert final["status"] == "completed", f"{course_id} did not complete: {final}"
    print(f"  PASS ✓ {course_id}")


def asw_script(s):
    # step 0: briefing
    s = act(s, "ack")
    show("ack", s)
    # step 1: select the frigate
    s = act(s, "select_unit", unit=s["unit"])
    show("select", s)
    # step 2: slow to 15 kt
    cmd({"cmd": "set_speed", "unit": s["unit"], "speed_kts": 15})
    s = advance(s, 5, "speed")
    show("speed", s)
    # step 3: wait for the towed array
    s = advance(s, 120, "towed")
    show("towed", s)
    # step 4: wait for the passive submarine contact
    s = advance(s, 1200, "sub contact")
    show("sub-contact", s)
    # step 5: launch the ASW helo from the carrier
    st = state()
    carrier = next(u for u in st["units"] if u["class"] == "cvn")
    cmd({"cmd": "launch_aircraft", "unit": carrier["id"], "class": "helo_asw",
         "count": 1, "mission": {"mission": "asw",
                                 "area": [carrier["pos"]["x"], carrier["pos"]["y"]],
                                 "radius_nm": 8}})
    s = advance(s, 10, "helo launch")
    show("helo", s)
    # step 6: fire ASW rockets at the submarine contact
    st = state()
    sub = next(c for c in st["contacts"] if c["category"] == "Sub" and not c["stale"])
    cmd({"cmd": "assign_target", "unit": s["unit"], "contact": sub["id"]})
    cmd({"cmd": "fire", "unit": s["unit"], "contact": sub["id"],
         "weapon": "asw_rocket", "rounds": 2})
    s = advance(s, 5, "asw fire")
    show("asw-fire", s)
    # step 7: debrief
    s = act(s, "ack")
    show("debrief", s)


def strike_script(s):
    s = act(s, "ack")
    show("ack", s)
    # surface contact (uses CV/DDG radars; targets at 14-16 nm)
    s = advance(s, 900, "contact")
    show("contact", s)
    st = state()
    contact = next(c for c in st["contacts"] if c["category"] == "Ship" and not c["stale"])
    cv = next(u for u in st["units"] if u["class"] == "cvn")
    # air strike
    cmd({"cmd": "launch_aircraft", "unit": cv["id"], "class": "striker", "count": 2,
         "mission": {"mission": "strike", "contact": contact["id"]}})
    s = advance(s, 20, "strike launch")
    show("air-strike", s)
    s = advance(s, 120, "air release")
    show("air-release", s)
    # surface strike
    st = state()
    ddg = next(u for u in st["units"] if u["class"] in ("ddg_aegis", "training_ddg_aa", "training_ddg_strike"))
    contact = next(c for c in st["contacts"] if c["category"] == "Ship" and not c["stale"])
    cmd({"cmd": "assign_target", "unit": ddg["id"], "contact": contact["id"]})
    cmd({"cmd": "fire", "unit": ddg["id"], "contact": contact["id"],
         "weapon": "ascm_std", "rounds": 1})
    s = advance(s, 5, "ship strike")
    show("ship-strike", s)
    # underwater strike
    st = state()
    sub = next(u for u in st["units"] if u["kind"] == "Sub")
    contact = next(c for c in st["contacts"] if c["category"] == "Ship" and not c["stale"])
    cmd({"cmd": "assign_target", "unit": sub["id"], "contact": contact["id"]})
    cmd({"cmd": "fire", "unit": sub["id"], "contact": contact["id"],
         "weapon": "torp_homing", "rounds": 1})
    s = advance(s, 5, "sub strike")
    show("sub-strike", s)
    s = act(s, "ack")
    show("debrief", s)


def defence_script(s):
    s = act(s, "ack")
    show("ack", s)
    # wait for the raid warning (the instructor AI fires when this step starts)
    s = advance(s, 900, "incoming")
    show("incoming", s)
    # chaff
    cmd({"cmd": "deploy_chaff", "unit": s["unit"]})
    s = advance(s, 5, "chaff")
    show("chaff", s)
    # turn the bow into the raid
    st = state()
    ddg = next(u for u in st["units"] if u["class"] in ("ddg_aegis", "training_ddg_aa", "training_ddg_strike"))
    missiles = [c for c in st["contacts"] if c["category"] == "Missile" and not c["stale"]]
    if missiles:
        m = missiles[0]["estimate"]
        import math
        deg = math.degrees(math.atan2(m["x"] - ddg["pos"]["x"], m["y"] - ddg["pos"]["y"])) % 360
    else:
        deg = 90
    cmd({"cmd": "set_course", "unit": ddg["id"], "course_deg": deg})
    s = advance(s, 5, "turn")
    show("turn", s)
    # weapons free
    cmd({"cmd": "set_weapons_free", "unit": ddg["id"], "free": True})
    s = advance(s, 5, "weapons free")
    show("weapons-free", s)
    # watch intercepts resolve
    s = advance(s, 600, "intercept")
    show("intercept", s)


run_course("asw", asw_script)
run_course("strike", strike_script)
run_course("fleet_defense", defence_script)
print("\nALL TRAINING LESSONS PASS ✓")
