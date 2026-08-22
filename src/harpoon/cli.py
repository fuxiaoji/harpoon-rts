from __future__ import annotations

import argparse
import json
from pathlib import Path

from harpoon.adapters import default_scenario_path, load_saved_game, load_scenario
from harpoon.domain.models import Command


def emit(value: object) -> None:
    if hasattr(value, "model_dump"):
        value = value.model_dump(mode="json")
    print(json.dumps(value, ensure_ascii=False, indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(prog="harpoon")
    parser.add_argument("--scenario", type=Path, default=default_scenario_path())
    parser.add_argument("--state", type=Path, help="从保存文件恢复，而不是新建想定")
    parser.add_argument("--write", type=Path, help="操作成功后写入此保存文件")
    sub = parser.add_subparsers(dest="operation", required=True)
    observe = sub.add_parser("observe")
    observe.add_argument("side")
    actions = sub.add_parser("actions")
    actions.add_argument("side")
    actions.add_argument("--unit")
    explain = sub.add_parser("explain")
    explain.add_argument("action_id")
    submit = sub.add_parser("submit")
    submit.add_argument("command_json")
    lock = sub.add_parser("lock")
    lock.add_argument("side")
    sub.add_parser("resolve")
    adjudicate = sub.add_parser("adjudicate")
    adjudicate.add_argument("entity_id")
    adjudicate.add_argument("--hit", action=argparse.BooleanOptionalAction, required=True)
    replay = sub.add_parser("replay")
    replay.add_argument("--jsonl", type=Path)
    sub.add_parser("snapshot")
    args = parser.parse_args()
    game = load_saved_game(args.state) if args.state else load_scenario(args.scenario)
    if args.operation == "observe":
        emit(game.observe(args.side))
    elif args.operation == "actions":
        emit([x.model_dump(mode="json") for x in game.list_actions(args.side, args.unit)])
    elif args.operation == "explain":
        emit(game.explain_action(args.action_id))
    elif args.operation == "submit":
        emit(game.submit(Command.model_validate_json(args.command_json)))
    elif args.operation == "lock":
        emit(game.lock(args.side))
    elif args.operation == "resolve":
        emit([event.model_dump(mode="json") for event in game.resolve()])
    elif args.operation == "adjudicate":
        emit(game.adjudicate_arrival(args.entity_id, args.hit))
    elif args.operation == "replay":
        if args.jsonl:
            game.export_jsonl(args.jsonl)
        emit([event.model_dump(mode="json") for event in game.events])
    elif args.operation == "snapshot":
        emit(game.snapshot())
    if args.write:
        game.save(args.write)


if __name__ == "__main__":
    main()
