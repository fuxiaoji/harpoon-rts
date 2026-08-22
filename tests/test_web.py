from __future__ import annotations

import json
import threading
from collections.abc import Iterator
from contextlib import contextmanager
from urllib.request import Request, urlopen

from harpoon.web import GameServer, create_server


@contextmanager
def running_server() -> Iterator[str]:
    server = create_server(port=0, state_path=None)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def get_json(url: str) -> object:
    with urlopen(url) as response:
        return json.loads(response.read())


def post_json(url: str, payload: dict[str, object]) -> object:
    request = Request(
        url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urlopen(request) as response:
        return json.loads(response.read())


def test_ui_and_side_scoped_state_are_served() -> None:
    with running_server() as base:
        with urlopen(base) as response:
            html = response.read().decode()
        with urlopen(f"{base}/app.js") as response:
            javascript = response.read().decode()
        state = get_json(f"{base}/api/state?side=nato")
        assert "HARPOON V" in html
        assert 'id="map-zoom-in"' in html
        assert 'id="combat-feedback"' in html
        assert "setMapZoom" in javascript
        assert "resolutionSteps" in javascript
        assert isinstance(state, dict)
        assert len(state["units"]) == 5
        assert all(unit["side_id"] == "nato" for unit in state["units"])


def test_ui_api_can_submit_and_lock_command() -> None:
    with running_server() as base:
        result = post_json(
            f"{base}/api/submit",
            {
                "command_id": "web-wait",
                "side_id": "nato",
                "action_id": "wait:poolster-a835",
                "observation_version": 0,
                "parameters": {},
            },
        )
        status = post_json(f"{base}/api/lock", {"side_id": "nato"})
        assert result["accepted"]
        assert status["locked_sides"] == ["nato"]


def test_server_type_is_explicit() -> None:
    server = create_server(port=0)
    try:
        assert isinstance(server, GameServer)
    finally:
        server.server_close()


def test_scenario_catalog_and_verified_ryukyu_data_are_exposed_safely() -> None:
    with running_server() as base:
        catalog = get_json(f"{base}/api/scenarios")
        assert isinstance(catalog, list)
        assert {item["scenario_id"] for item in catalog} >= {
            "tattletail-1991-strict",
            "ryukyu-shockwave-2025",
        }
        post_json(
            f"{base}/api/scenarios/load", {"scenario_id": "ryukyu-shockwave-2025"}
        )
        state = get_json(f"{base}/api/state?side=china")
        actions = get_json(f"{base}/api/actions?side=china")
        assert state["scenario_id"] == "ryukyu-shockwave-2025"
        assert len(state["units"]) == 3
        assert not state["ground_facilities"]
        land_strikes = [action for action in actions if action["action_type"] == "land_strike"]
        assert len(land_strikes) == 1
        assert land_strikes[0]["available"]
        assert "kadena-runway-05r23l" in land_strikes[0]["candidate_targets"]


def test_staff_api_requires_plan_and_fire_confirmation() -> None:
    with running_server() as base:
        plan = post_json(
            f"{base}/api/staff/intent",
            {
                "intent_id": "web-intent",
                "side_id": "soviet",
                "objective": "攻击已确认目标，然后保持阵位",
            },
        )
        approved = post_json(
            f"{base}/api/staff/plan/approve",
            {"side_id": "soviet", "plan_id": plan["plan_id"]},
        )
        assert approved["status"] == "approved"
        execution = post_json(f"{base}/api/staff/execute", {"side_id": "soviet"})
        assert execution["approvals"]
        assert not execution["submitted"]
        approval_id = execution["approvals"][0]["approval_id"]
        result = post_json(
            f"{base}/api/staff/action/approve",
            {"side_id": "soviet", "approval_id": approval_id},
        )
        assert result["result"]["accepted"]
        briefing = get_json(f"{base}/api/staff/briefing?side=soviet")
        assert briefing["mission_progress"].startswith("执行指挥意图")


def test_tutorial_opponent_submits_legal_attack_and_locks() -> None:
    with running_server() as base:
        result = post_json(f"{base}/api/tutorial/opponent", {})
        state = get_json(f"{base}/api/state?side=referee")
        assert result["locked"]
        assert len(result["commands"]) == 1
        assert result["commands"][0]["accepted"]
        assert "soviet" in state["locked_sides"]


def test_tutorial_salvo_can_complete_a_full_battle() -> None:
    with running_server() as base:
        post_json(f"{base}/api/lock", {"side_id": "nato"})
        post_json(f"{base}/api/tutorial/opponent", {})
        for _ in range(6):
            post_json(f"{base}/api/resolve", {})

        salvo = post_json(f"{base}/api/tutorial/salvo", {})
        assert len(salvo["commands"]) == 4
        post_json(f"{base}/api/lock", {"side_id": "nato"})
        post_json(f"{base}/api/tutorial/opponent", {})
        post_json(f"{base}/api/resolve", {})
        post_json(f"{base}/api/resolve", {})
        post_json(
            f"{base}/api/tutorial/adjudicate", {"owner_side_id": "soviet"}
        )
        for _ in range(4):
            post_json(f"{base}/api/resolve", {})

        post_json(f"{base}/api/lock", {"side_id": "nato"})
        post_json(f"{base}/api/tutorial/opponent", {})
        post_json(f"{base}/api/resolve", {})
        post_json(f"{base}/api/resolve", {})
        damage = post_json(
            f"{base}/api/tutorial/adjudicate", {"owner_side_id": "nato"}
        )
        assert len(damage["events"]) == 4
        hit_event = next(event for event in damage["events"] if event["payload"]["hit"])
        calculation = hit_event["payload"]["calculation"]
        assert calculation["formula"] == "base_damage_dp + floor(D6 / damage_divisor)"
        assert calculation["result_damage_dp"] >= 34
        assert hit_event["payload"]["hit_resolution"]["lookup"]["table_id"] == (
            "H5-8.3-ANTISHIP-MISSILE-ATTACK"
        )
        for _ in range(4):
            post_json(f"{base}/api/resolve", {})
        state = get_json(f"{base}/api/state?side=nato")
        assert state["winner"] in (None, "nato_decisive", "nato_tactical", "nato")
        assert any(event["event_type"] == "contact_solution_report" for event in state["events"])
