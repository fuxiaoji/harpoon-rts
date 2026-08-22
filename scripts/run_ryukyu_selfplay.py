from __future__ import annotations

import json
import threading
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

from harpoon.web import create_server

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts" / "replays" / "ryukyu-http-selfplay-seed-20251017"


def request_json(base: str, path: str, payload: dict[str, Any] | None = None) -> Any:
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    request = Request(
        base + path,
        data=data,
        headers={"Content-Type": "application/json"},
        method="GET" if payload is None else "POST",
    )
    with urlopen(request, timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))


def main() -> None:
    server = create_server(
        port=0,
        scenario=ROOT / "data" / "scenarios" / "ryukyu_shockwave_2025.json",
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base = f"http://127.0.0.1:{server.server_address[1]}"
    audit: list[dict[str, Any]] = []
    targets = [
        "kadena-runway-05r23l",
        "kadena-runway-05l23r",
        "naha-runway-18l36r",
        "naha-runway-18r36l",
        "futenma-runway-06-24",
    ]
    launches = 0
    try:
        for _ in range(240):
            state = request_json(base, "/api/state?side=china")
            if state["winner"]:
                break
            actions = request_json(base, "/api/actions?side=china&unit=plarf-cj10-battery")
            strike = next(
                (
                    item
                    for item in actions
                    if item["action_type"] == "land_strike" and item["available"]
                ),
                None,
            )
            if strike is not None and launches < 12:
                target = targets[launches % len(targets)]
                result = request_json(
                    base,
                    "/api/submit",
                    {
                        "command_id": f"china-strike-{launches + 1:02d}",
                        "side_id": "china",
                        "action_id": strike["action_id"],
                        "observation_version": state["version"],
                        "parameters": {"target_id": target},
                    },
                )
                audit.append({"kind": "command", "target": target, "result": result})
                launches += 1
            request_json(base, "/api/lock", {"side_id": "china"})
            request_json(base, "/api/lock", {"side_id": "usa"})
            for _phase in range(6):
                events = request_json(base, "/api/resolve", {})
                audit.extend({"kind": "event", "event": event} for event in events)
                current = request_json(base, "/api/state?side=referee")
                for weapon in current["in_flight_weapons"]:
                    if (
                        weapon["awaiting_adjudication"]
                        and weapon["target_kind"] == "ground_facility"
                    ):
                        event = request_json(
                            base, "/api/adjudicate/land", {"entity_id": weapon["entity_id"]}
                        )
                        audit.append({"kind": "event", "event": event})
            if request_json(base, "/api/state?side=referee")["winner"]:
                break
        final = request_json(base, "/api/state?side=referee")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "audit.jsonl").write_text(
        "".join(json.dumps(item, ensure_ascii=False, sort_keys=True) + "\n" for item in audit),
        encoding="utf-8",
    )
    (OUT / "final_state.json").write_text(
        json.dumps(final, ensure_ascii=False, indent=2, sort_keys=True), encoding="utf-8"
    )
    strikes = [
        item["event"]
        for item in audit
        if item["kind"] == "event" and item["event"]["event_type"] == "land_strike_adjudicated"
    ]
    facilities = final["ground_facilities"]
    lines = [
        "# 琉球震潮 HTTP 自战战报",
        "",
        "- 固定种子：20251017",
        f"- 结束时间：{final['timestamp']}，第 {final['turn']} 战术回合",
        f"- 胜者：{final['winner']}",
        f"- CJ-10 发射：{launches} 枚；已裁决：{len(strikes)} 枚；命中：{sum(bool(e['payload']['hit']) for e in strikes)} 枚",
        f"- 最终状态哈希：`{final['state_hash']}`",
        "",
        "## 基地毁伤",
        "",
    ]
    for facility in facilities:
        lines.append(
            f"- {facility['name']}：{facility['damage_dp']}/{facility['maximum_dp']} DP，"
            f"运行={'是' if facility['operational'] else '否'}，跑道切口={facility['runway_cuts']}，"
            f"来源类型={facility['provenance']}"
        )
    lines.extend(
        [
            "",
            "## 裁决说明",
            "",
            "每枚导弹均由HTTP合法行动提交；抵达后按11-3表定位导弹类型、制导等级和目标尺寸，记录D100、伤害D6、DP变化和规则ID。",
        ]
    )
    (OUT / "battle_report_zh.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(
        json.dumps(
            {
                "winner": final["winner"],
                "turn": final["turn"],
                "hash": final["state_hash"],
                "out": str(OUT),
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()
