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
        state = get_json(f"{base}/api/state?side=nato")
        assert "HARPOON V" in html
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
