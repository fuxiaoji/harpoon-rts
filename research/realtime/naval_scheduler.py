"""Serializable public-observation scheduler for the seven naval research arms.

This module does no IO, reads no World, and makes no model requests. All state
transitions return a copy. The caller must record the returned audit rows and
charge every ``start_request`` including initialization/recovery. Conditions are
OR clauses of AND comparisons, shared by human and model arms; no code is run.

Protocol: observe T0, start the common initialization request, then observe every
public sample while the world runs. Complete a request with finish_request; if
is_ready, dispatch using the latest observed feature snapshot. Event arms latch
busy-period events into one bounded summary; they do not get a hidden timer.
Fixed and hybrid grids are absolute phase + n * period and are never reset.

Persisting this state preserves an in-flight request ID, not the underlying API
connection. A restored caller must recover its actual receipt or explicitly fail
that same request before issuing another. Do not silently replay model calls.
"""
from __future__ import annotations

import copy
import math
import operator


VERSION = 1
POLICIES = ("F60", "F300", "F600", "EH", "EM", "HH600", "HM600")
REQUIRED_FIELDS = ("tick", "contact_revision", "contacts_count", "incoming_count",
                   "own_loss_count", "task_revision", "plan_age_s")
OPTIONAL_FIELDS = ("incoming_revision",)
FIELDS = REQUIRED_FIELDS + OPTIONAL_FIELDS
OPS = {"eq": operator.eq, "ne": operator.ne, "lt": operator.lt,
       "le": operator.le, "gt": operator.gt, "ge": operator.ge}
MAX_CLAUSES = 4
MAX_ATOMS = 3
MAX_NUMBER = 1_000_000_000_000
_MODEL = ("EM", "HM600")
_HUMAN = ("EH", "HH600")
_PERIODS = {"F60": 60, "F300": 300, "F600": 600,
            "HH600": 600, "HM600": 600}
_MONOTONE = ("contact_revision", "task_revision", "own_loss_count", "incoming_revision")


def _integer(value, name, minimum=0, maximum=MAX_NUMBER):
    if type(value) is not int or not minimum <= value <= maximum:
        raise ValueError(f"{name} must be an integer in [{minimum}, {maximum}]")
    return value


def validate_features(features):
    """Fail closed on unknown/private keys and copy the numerical projection."""
    if not isinstance(features, dict):
        raise ValueError("features must be a public numerical object")
    if set(features) - set(FIELDS) or set(REQUIRED_FIELDS) - set(features):
        raise ValueError("features must contain only the documented public fields")
    return {field: _integer(value, field) for field, value in features.items()}


def conditions_schema():
    """Independent JSON-schema fragment for the model's wake_conditions field."""
    return {"type": "array", "maxItems": MAX_CLAUSES, "items": {
        "type": "object", "required": ["all"], "additionalProperties": False,
        "properties": {"all": {"type": "array", "minItems": 1,
            "maxItems": MAX_ATOMS, "items": {"type": "object",
                "required": ["field", "op", "value"], "additionalProperties": False,
                "properties": {"field": {"type": "string", "enum": list(FIELDS)},
                    "op": {"type": "string", "enum": list(OPS)},
                    "value": {"type": "number", "minimum": -MAX_NUMBER,
                              "maximum": MAX_NUMBER}}}}}}}


def validate_conditions(conditions, available_fields=None):
    """Validate/copy a bounded numerical DSL; [] means deliberately no events."""
    if not isinstance(conditions, list) or len(conditions) > MAX_CLAUSES:
        raise ValueError("wake_conditions must be a list of at most four clauses")
    available = set(FIELDS if available_fields is None else available_fields)
    for clause in conditions:
        if not isinstance(clause, dict) or set(clause) != {"all"}:
            raise ValueError("a clause must contain only all")
        atoms = clause["all"]
        if not isinstance(atoms, list) or not 1 <= len(atoms) <= MAX_ATOMS:
            raise ValueError("an AND clause needs one to three comparisons")
        for atom in atoms:
            if not isinstance(atom, dict) or set(atom) != {"field", "op", "value"}:
                raise ValueError("a comparison needs only field, op, value")
            if atom["field"] not in FIELDS or atom["field"] not in available:
                raise ValueError("unknown or unavailable public condition field")
            if atom["op"] not in OPS:
                raise ValueError("unknown comparison operator")
            value = atom["value"]
            if (type(value) not in (int, float) or not math.isfinite(value)
                    or abs(value) > MAX_NUMBER):
                raise ValueError("a condition threshold must be finite and bounded")
    return copy.deepcopy(conditions)


def human_conditions(features):
    """Four explicit increment rules, using the same DSL offered to the model.

    incoming_revision should count newly observed incoming identities. Without
    that optional field, incoming_count cannot detect a same-count replacement.
    The caller must disclose this limitation rather than inject hidden truth.
    """
    f = validate_features(features)
    incoming = "incoming_revision" if "incoming_revision" in f else "incoming_count"
    return [{"all": [{"field": key, "op": "gt", "value": f[key]}]}
            for key in ("contact_revision", "task_revision", "own_loss_count", incoming)]


def new_state(policy, *, phase_s=0, max_initialization_retries=1,
              initialization_retry_delay_s=30):
    """Create a T0 scheduler. Retry allowance is only used by EM/HM bootstrap.

    A valid empty watch is initialized successfully and gets no recovery calls.
    Model initialization failures allow at most the explicitly configured number
    of extra requests. This is a receipt-based recovery, not an event fallback.
    """
    if policy not in POLICIES:
        raise ValueError("unknown naval scheduling policy")
    phase = _integer(phase_s, "phase_s")
    period = _PERIODS.get(policy)
    if (period is None and phase != 0) or (period is not None and phase >= period):
        raise ValueError("phase must be zero for event arms or smaller than the period")
    retries = _integer(max_initialization_retries, "max_initialization_retries", maximum=10)
    delay = _integer(initialization_retry_delay_s, "initialization_retry_delay_s", minimum=1)
    return {"version": VERSION, "policy": policy, "phase_s": phase,
            "last_tick": None, "last_features": None, "initialized": False,
            "conditions_installed": False, "conditions_source": None,
            "conditions": [], "watch_previous": [], "watch_keys": [],
            "conditions_generation": 0,
            "next_grid_tick": (phase or period) if period is not None else None,
            "pending": None, "inflight": None, "request_count": 0,
            "finished_count": 0, "issued_request_ids": [], "trigger_count": 0,
            "audit_sequence": 0, "retry_due_tick": None, "retry_used": 0,
            "max_initialization_retries": retries,
            "initialization_retry_delay_s": delay}


def _copy(state):
    if not isinstance(state, dict) or state.get("version") != VERSION or state.get("policy") not in POLICIES:
        raise ValueError("unsupported scheduler state")
    return copy.deepcopy(state)


def _audit(state, kind, **data):
    state["audit_sequence"] += 1
    return {"sequence": state["audit_sequence"], "kind": kind, **data}


def _wake(state, audit, features, reason, *, triggered_tick=None, last_tick=None,
          clause_ids=(), count=1):
    tick = features["tick"] if triggered_tick is None else triggered_tick
    last = tick if last_tick is None else last_tick
    row = _audit(state, "wake", reason=reason, triggered_tick=tick,
                 last_triggered_tick=last, observed_tick=features["tick"],
                 represented_event_count=count, clause_ids=list(clause_ids),
                 features=copy.deepcopy(features), busy=state["inflight"] is not None)
    audit.append(row)
    state["trigger_count"] += count
    if state["pending"] is None:
        state["pending"] = {"triggered_tick": tick, "last_triggered_tick": last,
            "reason": reason, "reasons": [reason], "event_count": count,
            "clause_ids": list(clause_ids), "first_features": copy.deepcopy(features),
            "latest_features": copy.deepcopy(features)}
    else:
        pending = state["pending"]
        pending["triggered_tick"] = min(pending["triggered_tick"], tick)
        pending["last_triggered_tick"] = max(pending["last_triggered_tick"], last)
        pending["event_count"] += count
        pending["reasons"] = sorted(set(pending["reasons"]) | {reason})
        pending["clause_ids"] = sorted(set(pending["clause_ids"]) | set(clause_ids))
        pending["latest_features"] = copy.deepcopy(features)


def _identity(clause, features):
    fields = {atom["field"] for atom in clause["all"]}
    keys = set()
    if fields & {"contact_revision", "contacts_count"}:
        keys.add("contact_revision")
    if fields & {"incoming_revision", "incoming_count"}:
        keys.add("incoming_revision" if "incoming_revision" in features else "incoming_count")
    if "task_revision" in fields:
        keys.add("task_revision")
    if "own_loss_count" in fields:
        keys.add("own_loss_count")
    # Pure clock/age conditions have ordinary false-to-true edges, not one event
    # per second. Lists rather than tuples survive a JSON round trip unchanged.
    return [[key, features[key]] for key in sorted(keys)]


def _poll_watch(state, features, audit, *, installation=False):
    if not state["conditions_installed"]:
        return
    hits = []
    for index, clause in enumerate(state["conditions"]):
        truth = all(OPS[atom["op"]](features[atom["field"]], atom["value"])
                    for atom in clause["all"])
        identity = _identity(clause, features)
        old_key = state["watch_keys"][index]
        # In a count-only incoming projection, decreases must not masquerade as
        # newly observed incoming missiles. A proper incoming_revision solves
        # the distinct same-count replacement case explicitly.
        changed = bool(identity) and old_key != identity
        if identity and old_key and identity[0][0] == "incoming_count":
            changed = any(new[1] > old[1] for new, old in zip(identity, old_key))
        if truth and (not state["watch_previous"][index] or changed):
            hits.append(index)
            state["watch_keys"][index] = identity
        state["watch_previous"][index] = truth
    if hits:
        reason = "human_condition" if state["conditions_source"] == "human" else "model_condition"
        _wake(state, audit, features, reason, clause_ids=hits)
        if installation:
            audit[-1]["on_installation"] = True


def _install(state, conditions, source):
    state["conditions"] = copy.deepcopy(conditions)
    state["conditions_installed"] = True
    state["conditions_source"] = source
    state["conditions_generation"] += 1
    state["watch_previous"] = [False] * len(conditions)
    state["watch_keys"] = [None] * len(conditions)


def observe(state, features):
    """Sample public features, update absolute clocks, and latch all wake causes.

    Equal-tick repeated samples are idempotent unless their public event revision
    changes. Jumps coalesce missed grid points, keeping the actual schedule ticks
    and the later observation tick distinct. No model work is started here.
    """
    f = validate_features(features)
    result = _copy(state)
    previous = result["last_features"]
    if previous is None:
        if f["tick"] != 0:
            raise ValueError("a fresh scheduler must observe the common T0 initialization")
    else:
        if f["tick"] < previous["tick"]:
            raise ValueError("public observation tick cannot go backwards")
        if set(f) != set(previous):
            raise ValueError("public feature availability cannot change within an episode")
        for key in _MONOTONE:
            if key in f and f[key] < previous[key]:
                raise ValueError(f"public event identity {key} cannot go backwards")
    result["last_tick"] = f["tick"]
    result["last_features"] = f
    audit = []
    if previous is None:
        if result["policy"] in _HUMAN:
            _install(result, human_conditions(f), "human")
        _wake(result, audit, f, "initialization")
    next_tick = result["next_grid_tick"]
    if next_tick is not None and next_tick <= f["tick"]:
        period = _PERIODS[result["policy"]]
        count = (f["tick"] - next_tick) // period + 1
        last = next_tick + (count - 1) * period
        reason = "fixed_grid" if result["policy"].startswith("F") else "fallback_grid"
        _wake(result, audit, f, reason, triggered_tick=next_tick, last_tick=last, count=count)
        result["next_grid_tick"] = last + period
    _poll_watch(result, f, audit)
    due = result["retry_due_tick"]
    if due is not None and f["tick"] >= due:
        _wake(result, audit, f, "initialization_recovery", triggered_tick=due)
        result["retry_due_tick"] = None
    return result, audit


def is_ready(state):
    return state["pending"] is not None and state["inflight"] is None


def start_request(state, request_id, features):
    """Consume the pending summary and bind one request to its actual snapshot.

    The runner is responsible for the real call/cost receipt. Request IDs must
    be unique within the episode, including attempts that fail or time out.
    """
    f = validate_features(features)
    result = _copy(state)
    if type(request_id) not in (int, str) or (isinstance(request_id, str) and not 1 <= len(request_id) <= 128):
        raise ValueError("request_id must be an integer or a nonempty bounded string")
    if request_id in result["issued_request_ids"]:
        raise ValueError("request IDs cannot be reused")
    if f != result["last_features"]:
        raise ValueError("observe the current public features before dispatch")
    if not is_ready(result):
        raise ValueError("no pending wake or a request is already in flight")
    pending = result["pending"]
    record = {"request_id": request_id, "ordinal": result["request_count"],
              "requested_tick": f["tick"], "features": f,
              "triggered_tick": pending["triggered_tick"],
              "last_triggered_tick": pending["last_triggered_tick"],
              "reason": pending["reason"], "reasons": pending["reasons"],
              "event_count": pending["event_count"], "clause_ids": pending["clause_ids"],
              "initialization": not result["initialized"]}
    result["request_count"] += 1
    result["issued_request_ids"].append(request_id)
    result["inflight"] = copy.deepcopy(record)
    result["pending"] = None
    # A genuine hybrid grid may supply a new initialization opportunity before
    # a delayed bootstrap retry is due. That request supersedes the old timer.
    if not result["initialized"]:
        result["retry_due_tick"] = None
    if result["policy"] in _HUMAN:
        _install(result, human_conditions(f), "human")
    return result, record


def finish_request(state, request_id, features, *, success, wake_conditions=None,
                   failure_reason="request_failed"):
    """Consume one receipt; stale/duplicate IDs cannot install watches or act.

    ``success`` must include the caller's action validation/application decision.
    A failed/stale action must use success=False even if its watch parses. A valid
    model watch is evaluated immediately on the current public sample, including
    conditions that became true while it was being generated. Empty watches are
    valid. Missing/invalid watches raise before consuming the matching receipt.
    """
    result = _copy(state)
    active = result["inflight"]
    if active is None or active["request_id"] != request_id:
        return result, [_audit(result, "response_ignored", request_id=request_id,
                               reason="not_current_inflight_request")]
    if type(success) is not bool:
        raise ValueError("success must be boolean")
    f = validate_features(features)
    conditions = None
    if success and result["policy"] in _MODEL:
        conditions = validate_conditions(wake_conditions, f)
    result, audit = observe(result, f)
    result["inflight"] = None
    result["finished_count"] += 1
    audit.append(_audit(result, "request_finished", request_id=request_id,
                        requested_tick=active["requested_tick"], response_tick=f["tick"],
                        success=success, failure_reason=None if success else str(failure_reason)))
    if success:
        result["initialized"] = True
        result["retry_due_tick"] = None
        if result["policy"] in _MODEL:
            _install(result, conditions, "model")
            audit.append(_audit(result, "conditions_installed", request_id=request_id,
                                tick=f["tick"], generation=result["conditions_generation"],
                                conditions=copy.deepcopy(conditions),
                                input_tick=active["requested_tick"]))
            _poll_watch(result, f, audit, installation=True)
    elif (result["policy"] in _MODEL and not result["initialized"]
          and result["retry_used"] < result["max_initialization_retries"]):
        result["retry_used"] += 1
        result["retry_due_tick"] = f["tick"] + result["initialization_retry_delay_s"]
        audit.append(_audit(result, "recovery_scheduled", due_tick=result["retry_due_tick"],
                            retry_ordinal=result["retry_used"],
                            maximum=result["max_initialization_retries"]))
    return result, audit


def restore_state(payload):
    """Validate a JSON-loaded checkpoint; never turn an in-flight call into ready."""
    result = _copy(payload)
    template = new_state(result["policy"], phase_s=result["phase_s"],
                         max_initialization_retries=result["max_initialization_retries"],
                         initialization_retry_delay_s=result["initialization_retry_delay_s"])
    if set(result) != set(template):
        raise ValueError("scheduler checkpoint fields do not match this version")
    for key in ("initialized", "conditions_installed"):
        if type(result[key]) is not bool:
            raise ValueError(f"checkpoint {key} must be boolean")
    for key in ("request_count", "finished_count", "trigger_count", "audit_sequence",
                "retry_used", "conditions_generation"):
        _integer(result[key], key)
    if result["retry_used"] > result["max_initialization_retries"]:
        raise ValueError("checkpoint exceeds initialization retry limit")
    if (len(result["issued_request_ids"]) != result["request_count"]
            or len(set(result["issued_request_ids"])) != result["request_count"]):
        raise ValueError("checkpoint request IDs are inconsistent")
    busy = int(result["inflight"] is not None)
    if result["finished_count"] + busy != result["request_count"]:
        raise ValueError("checkpoint request ledger is inconsistent")
    f = result["last_features"]
    if f is None:
        if result != template:
            raise ValueError("unobserved checkpoint has changed state")
        return result
    f = validate_features(f)
    if result["last_tick"] != f["tick"]:
        raise ValueError("checkpoint observation ticks disagree")
    conditions = validate_conditions(result["conditions"], f)
    if len(result["watch_previous"]) != len(conditions) or len(result["watch_keys"]) != len(conditions):
        raise ValueError("checkpoint watch state has incorrect size")
    if any(type(item) is not bool for item in result["watch_previous"]):
        raise ValueError("checkpoint watch truth values must be boolean")
    source = result["conditions_source"]
    expected = "human" if result["policy"] in _HUMAN else "model" if result["policy"] in _MODEL else None
    if (result["conditions_installed"] and source != expected) or (not result["conditions_installed"] and source is not None):
        raise ValueError("checkpoint condition source does not match its policy")
    if result["policy"] in _MODEL and result["initialized"] and not result["conditions_installed"]:
        raise ValueError("initialized model event checkpoint must have its watch installed")
    period = _PERIODS.get(result["policy"])
    grid = result["next_grid_tick"]
    if period is None:
        if grid is not None:
            raise ValueError("pure event checkpoint must not have a timer")
    elif (type(grid) is not int or grid <= f["tick"]
          or (grid - result["phase_s"]) % period != 0):
        raise ValueError("checkpoint absolute grid is inconsistent")
    due = result["retry_due_tick"]
    if due is not None:
        if (result["policy"] not in _MODEL or result["initialized"]
                or type(due) is not int or due <= f["tick"] or result["retry_used"] == 0):
            raise ValueError("checkpoint recovery timer is inconsistent")
    for record in (result["pending"], result["inflight"]):
        if record is not None:
            if not 0 <= record["triggered_tick"] <= record["last_triggered_tick"] <= f["tick"]:
                raise ValueError("checkpoint trigger times are inconsistent")
            _integer(record["event_count"], "event_count", minimum=1)
    if result["inflight"] is not None:
        active = result["inflight"]
        if (active["request_id"] != result["issued_request_ids"][-1]
                or active["ordinal"] != result["request_count"] - 1
                or active["features"]["tick"] != active["requested_tick"]
                or active["requested_tick"] > f["tick"]):
            raise ValueError("checkpoint active request does not match the ledger")
        validate_features(active["features"])
    return result


def dump_state(state):
    """Return a validated JSON-compatible copy for checkpoint serialization."""
    return restore_state(state)
