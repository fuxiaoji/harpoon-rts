from __future__ import annotations

import argparse
import json
import os
import threading
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse

from pydantic import ValidationError

from harpoon.adapters import (
    default_scenario_path,
    load_saved_game,
    load_scenario,
    scenario_catalog,
    scenario_path,
)
from harpoon.ai import CommanderIntent, CommandStaff, DeepSeekChatProvider
from harpoon.ai.command_staff import StaffProvider
from harpoon.controllers.scripted import FireFirstController, WaitController
from harpoon.domain.models import Command

ROOT = Path(__file__).resolve().parents[2]
UI_ROOT = ROOT / "ui"


class GameApplication:
    def __init__(
        self,
        scenario: Path,
        state_path: Path | None = None,
        staff_provider: StaffProvider | None = None,
    ) -> None:
        self.scenario_path = scenario
        self.state_path = state_path
        self.game = (
            load_saved_game(state_path)
            if state_path and state_path.exists()
            else load_scenario(scenario)
        )
        self.staff_provider = staff_provider
        self.staff = CommandStaff(staff_provider)
        self.active_plans: dict[str, str] = {}
        self.lock = threading.RLock()

    def reset(self) -> None:
        self.game = load_scenario(self.scenario_path)
        self.staff = CommandStaff(self.staff_provider)
        self.active_plans.clear()
        self.persist()

    def load_named_scenario(self, scenario_id: str) -> None:
        self.scenario_path = scenario_path(scenario_id)
        self.reset()

    def issue_staff_intent(self, payload: dict[str, Any]) -> dict[str, Any]:
        intent = CommanderIntent.model_validate(payload)
        observation = self.game.observe(intent.side_id)
        plan = self.staff.issue_intent(intent, observation, self.game.list_actions(intent.side_id))
        self.active_plans[intent.side_id] = plan.plan_id
        return plan.model_dump(mode="json")

    def approve_staff_plan(self, side_id: str, plan_id: str) -> dict[str, Any]:
        plan = self.staff.approve_plan(plan_id, side_id)
        return plan.model_dump(mode="json")

    def execute_staff_plan(self, side_id: str) -> dict[str, Any]:
        plan_id = self.active_plans[side_id]
        observation = self.game.observe(side_id)
        proposals = self.staff.propose(plan_id, observation, self.game.list_actions(side_id))
        commands, approvals = self.staff.commands_or_approvals(proposals)
        results = [self.game.submit(command).model_dump(mode="json") for command in commands]
        return {
            "proposals": [item.model_dump(mode="json") for item in proposals],
            "submitted": results,
            "approvals": [item.model_dump(mode="json") for item in approvals],
        }

    def approve_staff_action(self, side_id: str, approval_id: str) -> dict[str, Any]:
        request = self.staff.approvals[approval_id]
        if request.proposal.side_id != side_id:
            raise ValueError("不能批准其他阵营行动")
        observation = self.game.observe(side_id)
        if request.proposal.observation_version != observation.version:
            raise ValueError("行动提议已过期，请重新生成")
        approved = request.model_copy(update={"status": "approved"})
        self.staff.approvals[approval_id] = approved
        commands, _ = self.staff.commands_or_approvals(
            (request.proposal,), frozenset({approval_id})
        )
        result = self.game.submit(commands[0])
        return {"approval": approved.model_dump(mode="json"), "result": result.model_dump(mode="json")}

    def staff_briefing(self, side_id: str) -> dict[str, Any]:
        plan_id = self.active_plans.get(side_id)
        return self.staff.briefing(self.game.observe(side_id), plan_id).model_dump(mode="json")

    def persist(self) -> None:
        if self.state_path:
            self.state_path.parent.mkdir(parents=True, exist_ok=True)
            self.game.save(self.state_path)

    def run_tutorial_opponent(self) -> list[dict[str, Any]]:
        side_id = "soviet"
        if side_id in self.game.state.locked_sides:
            return []
        observation = self.game.observe(side_id)
        controller = FireFirstController() if self.game.state.turn == 0 else WaitController()
        commands = controller.choose(observation, self.game.list_actions(side_id))
        results = []
        for command in commands:
            result = self.game.submit(command)
            if not result.accepted:
                raise RuntimeError("教学 AI 无法提交合法命令：" + "；".join(result.errors))
            results.append(result.model_dump(mode="json"))
        self.game.lock(side_id)
        return results

    def prepare_tutorial_salvo(self) -> list[dict[str, Any]]:
        observation = self.game.observe("nato")
        option = next(
            item
            for item in self.game.list_actions("nato", "uss-copeland-ffg25")
            if item.action_type == "fire" and item.available
        )
        results = []
        for index in range(4):
            result = self.game.submit(
                Command(
                    command_id=f"tutorial-salvo-{self.game.state.turn}-{index + 1}",
                    side_id="nato",
                    action_id=option.action_id,
                    observation_version=observation.version,
                    parameters={"target_id": option.candidate_targets[0]},
                )
            )
            if not result.accepted:
                raise RuntimeError("教学齐射无法提交：" + "；".join(result.errors))
            results.append(result.model_dump(mode="json"))
        return results

    def adjudicate_tutorial_arrivals(self, owner_side_id: str) -> list[dict[str, Any]]:
        arrivals = [
            weapon
            for weapon in self.game.state.in_flight_weapons
            if weapon.awaiting_adjudication and weapon.owner_side_id == owner_side_id
        ]
        if not arrivals:
            raise RuntimeError("当前没有该方待裁决的抵达武器")
        return [
            self.game.auto_adjudicate_arrival(weapon.entity_id).model_dump(mode="json")
            for weapon in arrivals
        ]

    def view(self, side_id: str) -> dict[str, Any]:
        if side_id == "referee":
            units = [unit.model_dump(mode="json") for unit in self.game.state.units.values()]
            contacts = []
            events = [event.model_dump(mode="json") for event in self.game.events]
        else:
            observation = self.game.observe(side_id)
            units = [unit.model_dump(mode="json") for unit in observation.own_units]
            contacts = [contact.model_dump(mode="json") for contact in observation.contacts]
            events = [event.model_dump(mode="json") for event in observation.visible_events]
        return {
            "side_id": side_id,
            "scenario_id": self.game.state.scenario_id,
            "timestamp": self.game.state.timestamp.isoformat(),
            "turn": self.game.state.turn,
            "phase": self.game.state.phase.value,
            "version": self.game.state.version,
            "state_hash": self.game.state.state_hash(),
            "locked_sides": sorted(self.game.state.locked_sides),
            "winner": self.game.state.winner,
            "units": units,
            "platforms": {
                platform_id: {
                    "name": platform.name,
                    "maximum_dp": platform.maximum_dp,
                    "maximum_speed_knots": platform.maximum_speed_knots,
                }
                for platform_id, platform in self.game.platforms.items()
            },
            "contacts": contacts,
            "events": events,
            "objectives": list(self.game.objectives.get(side_id, ())),
            "in_flight_weapons": [
                weapon.model_dump(mode="json")
                for weapon in self.game.state.in_flight_weapons
                if side_id == "referee"
                or weapon.owner_side_id == side_id
                or (
                    weapon.target_kind == "unit"
                    and self.game.state.units[weapon.target_id].side_id == side_id
                )
                or (
                    weapon.target_kind == "ground_facility"
                    and self.game.ground_facilities[weapon.target_id].side_id == side_id
                )
            ],
            "ground_facilities": [
                {
                    **definition.model_dump(mode="json"),
                    **self.game.state.ground_facilities[facility_id].model_dump(mode="json"),
                }
                for facility_id, definition in self.game.ground_facilities.items()
                if side_id == "referee" or definition.side_id == side_id
            ],
            "pending_commands": len(self.game._commands) if side_id == "referee" else None,
            "available_sides": sorted(self.game.objectives),
            "staff_plan_id": self.active_plans.get(side_id),
        }


class Handler(BaseHTTPRequestHandler):
    server: GameServer

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/api/state":
            side = parse_qs(parsed.query).get("side", ["nato"])[0]
            self.json_response(self.server.app.view(side))
            return
        if parsed.path == "/api/scenarios":
            self.json_response([item.model_dump(mode="json") for item in scenario_catalog()])
            return
        if parsed.path == "/api/staff/briefing":
            side = parse_qs(parsed.query).get("side", [""])[0]
            self.json_response(self.server.app.staff_briefing(side))
            return
        if parsed.path == "/api/actions":
            query = parse_qs(parsed.query)
            side = query.get("side", ["nato"])[0]
            unit = query.get("unit", [None])[0]
            if side == "referee":
                self.json_response([])
            else:
                self.json_response(
                    [
                        item.model_dump(mode="json")
                        for item in self.server.app.game.list_actions(side, unit)
                    ]
                )
            return
        if parsed.path.startswith("/api/"):
            self.json_response({"error": "未知 API"}, HTTPStatus.NOT_FOUND)
            return
        self.serve_asset(parsed.path)

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        try:
            payload = self.read_json()
            with self.server.app.lock:
                result = self.mutate(parsed.path, payload)
                self.server.app.persist()
            self.json_response(result)
        except (KeyError, ValueError, RuntimeError, ValidationError) as exc:
            self.json_response({"error": str(exc)}, HTTPStatus.BAD_REQUEST)

    def mutate(self, path: str, payload: dict[str, Any]) -> Any:
        game = self.server.app.game
        if path == "/api/submit":
            return game.submit(Command.model_validate(payload)).model_dump(mode="json")
        if path == "/api/lock":
            return game.lock(str(payload["side_id"])).model_dump(mode="json")
        if path == "/api/resolve":
            return [event.model_dump(mode="json") for event in game.resolve()]
        if path == "/api/adjudicate":
            return game.adjudicate_arrival(
                str(payload["entity_id"]), bool(payload["hit"])
            ).model_dump(mode="json")
        if path == "/api/adjudicate/land":
            return game.auto_adjudicate_land_arrival(
                str(payload["entity_id"])
            ).model_dump(mode="json")
        if path == "/api/reset":
            self.server.app.reset()
            return {"reset": True}
        if path == "/api/scenarios/load":
            self.server.app.load_named_scenario(str(payload["scenario_id"]))
            return {"scenario_id": self.server.app.game.state.scenario_id, "loaded": True}
        if path == "/api/staff/intent":
            return self.server.app.issue_staff_intent(payload)
        if path == "/api/staff/plan/approve":
            return self.server.app.approve_staff_plan(
                str(payload["side_id"]), str(payload["plan_id"])
            )
        if path == "/api/staff/execute":
            return self.server.app.execute_staff_plan(str(payload["side_id"]))
        if path == "/api/staff/action/approve":
            return self.server.app.approve_staff_action(
                str(payload["side_id"]), str(payload["approval_id"])
            )
        if path == "/api/tutorial/opponent":
            return {"commands": self.server.app.run_tutorial_opponent(), "locked": True}
        if path == "/api/tutorial/salvo":
            return {"commands": self.server.app.prepare_tutorial_salvo()}
        if path == "/api/tutorial/adjudicate":
            return {
                "events": self.server.app.adjudicate_tutorial_arrivals(
                    str(payload["owner_side_id"])
                )
            }
        raise KeyError("未知 API")

    def serve_asset(self, request_path: str) -> None:
        relative = "index.html" if request_path in ("", "/") else request_path.lstrip("/")
        candidate = (UI_ROOT / relative).resolve()
        if UI_ROOT.resolve() not in candidate.parents and candidate != UI_ROOT.resolve():
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        if not candidate.is_file():
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        mime = {
            ".html": "text/html; charset=utf-8",
            ".css": "text/css; charset=utf-8",
            ".js": "text/javascript; charset=utf-8",
        }.get(candidate.suffix, "application/octet-stream")
        data = candidate.read_bytes()
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length > 1_000_000:
            raise ValueError("请求过大")
        return json.loads(self.rfile.read(length).decode("utf-8")) if length else {}

    def json_response(self, value: Any, status: HTTPStatus = HTTPStatus.OK) -> None:
        data = json.dumps(value, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, format: str, *args: object) -> None:
        return


class GameServer(ThreadingHTTPServer):
    def __init__(self, address: tuple[str, int], app: GameApplication) -> None:
        super().__init__(address, Handler)
        self.app = app


def create_server(
    host: str = "127.0.0.1",
    port: int = 8897,
    scenario: Path | None = None,
    state_path: Path | None = None,
    staff_provider: StaffProvider | None = None,
) -> GameServer:
    app = GameApplication(scenario or default_scenario_path(), state_path, staff_provider)
    return GameServer((host, port), app)


def main() -> None:
    parser = argparse.ArgumentParser(prog="harpoon-ui")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8897)
    parser.add_argument("--scenario", type=Path, default=default_scenario_path())
    parser.add_argument("--state", type=Path, default=Path("tmp/ui-session.json"))
    parser.add_argument("--llm-provider", choices=("deterministic", "deepseek"), default="deterministic")
    parser.add_argument("--llm-model", default="deepseek-v4-flash")
    args = parser.parse_args()
    provider: StaffProvider | None = None
    if args.llm_provider == "deepseek":
        api_key = os.environ.get("DEEPSEEK_API_KEY", "")
        if not api_key:
            raise SystemExit("使用 DeepSeek 时必须在进程环境提供 DEEPSEEK_API_KEY")
        provider = DeepSeekChatProvider(api_key=api_key, model=args.llm_model)
    server = create_server(args.host, args.port, args.scenario, args.state, provider)
    print(f"Harpoon UI: http://{args.host}:{args.port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
