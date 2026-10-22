"""Side-only transport for externally authored decisions; contains no game strategy.

Observe produces a fresh template. The decision author must retain its identity.
Advance never attaches a fresh observation to an old plan or retries a mutation.
"""
import argparse
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path


def validate_plan(plan):
    if not isinstance(plan, dict):
        raise ValueError("plan must be an object")
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,96}", str(plan.get("plan_id", ""))):
        raise ValueError("retain the observation template's plan_id")
    for key in ("expected_tick", "session_revision", "ticks"):
        if type(plan.get(key)) is not int:
            raise ValueError(f"{key} must be an explicit integer from the authored plan")
    if plan["expected_tick"] < 0 or plan["session_revision"] < 0 or not 1 <= plan["ticks"] <= 300:
        raise ValueError("invalid tick/session bounds")
    commands = plan.get("commands", [])
    if not isinstance(commands, list) or len(commands) > 16:
        raise ValueError("commands must be an array of at most 16 entries")
    brief = plan.get("brief")
    if not isinstance(brief, dict) or any(not isinstance(brief.get(k), str)
            for k in ("evidence", "options", "choice", "risk", "check")):
        raise ValueError("brief needs evidence/options/choice/risk/check strings")
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True, help="dedicated paused referee service")
    parser.add_argument("--side", choices=["Blue", "Red"], required=True)
    parser.add_argument("--output", type=Path, required=True)
    sub = parser.add_subparsers(dest="action", required=True)
    sub.add_parser("observe")
    advance = sub.add_parser("advance")
    advance.add_argument("plan", type=Path)
    args = parser.parse_args()
    url = urllib.parse.urlsplit(args.url)
    if url.scheme != "http" or url.hostname not in ("127.0.0.1", "localhost", "::1") or url.username or url.password:
        parser.error("use a local http service without embedded credentials")
    args.output.mkdir(parents=True, exist_ok=True)

    def api(path, body=None):
        request = urllib.request.Request(args.url.rstrip("/") + path,
            data=None if body is None else json.dumps(body, ensure_ascii=False).encode("utf-8"),
            headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.load(response)

    def save(path, value):
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    if args.action == "observe":
        observation = api(f"/api/state?side={args.side}&max_events=500")
        save(args.output / "latest-observation.json", observation)
        template = dict(plan_id=uuid.uuid4().hex, expected_tick=observation["tick"],
            session_revision=observation["session_revision"], ticks=300, commands=[],
            brief={key: "" for key in ("evidence", "options", "choice", "risk", "check")})
        path = args.output / f"plan-{template['plan_id']}.json"
        save(path, template)
        print(json.dumps({"tick":observation["tick"], "template":str(path),
                          "observation":str(args.output / "latest-observation.json")}, ensure_ascii=False))
        return

    plan = validate_plan(json.loads(args.plan.read_text(encoding="utf-8-sig")))
    observation = api(f"/api/state?side={args.side}&max_events=500")
    if (observation["tick"], observation["session_revision"]) != (plan["expected_tick"], plan["session_revision"]):
        raise ValueError("stale plan: observe and author a new decision; old identity was not refreshed")
    folder = args.output / plan["plan_id"]
    # Exclusive creation also protects retries after a network timeout. Inspect
    # the receipt/current state; never replay a potentially accepted decision.
    folder.mkdir(exist_ok=False)
    request = {key: plan[key] for key in ("plan_id", "expected_tick", "session_revision", "ticks")}
    request.update(side=args.side, commands=plan.get("commands", []),
                   intent=json.dumps(plan["brief"], ensure_ascii=False))
    save(folder / "before.json", observation)
    save(folder / "brief.json", plan["brief"])
    save(folder / "request.json", request)
    try:
        response = api("/api/advance", request)
    except urllib.error.HTTPError as error:
        save(folder / "rejected.json", {"status":error.code, "detail":error.read().decode("utf-8", errors="replace")})
        raise
    save(folder / "response.json", response)
    save(args.output / "latest-observation.json", response["observation"])
    print(json.dumps({"plan_id":plan["plan_id"], "start_tick":response["start_tick"],
                      "end_tick":response["end_tick"], "accepted":len(response["accepted"]),
                      "rejected":response["rejected"]}, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, urllib.error.URLError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
