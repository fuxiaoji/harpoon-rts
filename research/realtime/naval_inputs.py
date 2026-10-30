"""Public naval observations -> common scheduler features and model inputs.

This module never accepts a World or referee checkpoint. Event identities are
derived only from the player's successive observations, not exact enemy truth.
"""
from __future__ import annotations

import copy
import json
import math

FIELDS = ('tick', 'contact_revision', 'contacts_count', 'incoming_count',
          'incoming_revision', 'own_loss_count', 'task_revision', 'plan_age_s')


class PublicFeatures:
    def __init__(self, state=None):
        self.state = copy.deepcopy(state) if state is not None else {
            'contact_revision': 0, 'incoming_revision': 0, 'task_revision': 0,
            'contacts': None, 'incoming': None, 'tasks': None,
            'last_plan_tick': 0,
        }

    def update(self, obs):
        contacts = sorted((c['id'], c['category'], c.get('class_name'),
                           c.get('stale', False), c.get('bearing_only', False))
                          for c in obs['contacts'])
        incoming = sorted(c['id'] for c in obs['contacts']
                          if c['category'] == 'Missile' and not c.get('stale', False))
        # Completion/recall and inventory changes are observable. Motion, HP
        # noise and second-by-second endurance do not generate event storms.
        tasks = sorted((u['id'], u.get('mission'), u.get('assigned'),
                        tuple(tuple(x) for x in (u.get('deck') or {}).get('ready', [])),
                        tuple(sorted((u.get('functional_damage') or {}).items())))
                       for u in obs['units'])
        for key, current, revision in [('contacts', contacts, 'contact_revision'),
                                       ('incoming', incoming, 'incoming_revision'),
                                       ('tasks', tasks, 'task_revision')]:
            normalized = json.loads(json.dumps(current))
            if self.state[key] is not None and self.state[key] != normalized:
                self.state[revision] += 1
            self.state[key] = normalized
        return {k: v for k, v in {
            'tick': obs['tick'],
            'contact_revision': self.state['contact_revision'],
            'contacts_count': len(obs['contacts']),
            'incoming_count': len(incoming),
            'incoming_revision': self.state['incoming_revision'],
            'own_loss_count': (obs.get('victory') or {}).get('own_losses', 0),
            'task_revision': self.state['task_revision'],
            'plan_age_s': obs['tick'] - self.state['last_plan_tick'],
        }.items() if k in FIELDS}

    def accepted(self, tick):
        self.state['last_plan_tick'] = tick


def project_input(obs, features, public_db, last_decision=None, own_execution=None):
    """Whitelist both top-level input and observation; no directory/arm/seed."""
    selected = ('tick', 'time_s', 'your_side', 'over', 'result', 'weather',
                'day_night', 'rules_profile', 'control_version', 'units',
                'contacts', 'events', 'victory', 'mission', 'exercise')
    own_side = obs['your_side']
    if any(u['side'] != own_side for u in obs['units']):
        raise ValueError('observation contains non-own units')
    if any('about' in c or 'offset' in c for c in obs['contacts']):
        raise ValueError('contact contains referee identity')
    return {
        'observation': {k: copy.deepcopy(obs[k]) for k in selected if k in obs},
        'scheduler_features': {k: features[k] for k in FIELDS},
        'public_database': copy.deepcopy(public_db),
        'last_accepted_decision': copy.deepcopy(last_decision),
        'own_execution': copy.deepcopy(own_execution),
        'controller_contract': {
            'max_commands': 16, 'max_observation_age_s': 600,
            'world_continues_during_reasoning': True,
            'old_orders_continue': True, 'task_review_lease_s': 1800,
            'shared_maintenance_interval_s': 30,
            'wake_clauses_max': 4, 'wake_atoms_per_clause_max': 3,
            'wake_fields': list(FIELDS), 'wake_operators': ['eq', 'ne', 'gt', 'ge', 'lt', 'le'],
        },
    }


def script_conditions(features):
    return [{'all': [{'field': name, 'op': 'gt', 'value': features[name]}]}
            for name in ('contact_revision', 'incoming_revision', 'own_loss_count', 'task_revision')]


COMMAND_REFERENCE = """Commands are JSON objects tagged by cmd. All unit IDs must
be own live units and all contact IDs must come from the supplied contacts.
Supported examples (replace all IDs/values using the observation):
{"cmd":"set_speed","unit":1,"speed_kts":14}
{"cmd":"set_course","unit":1,"course_deg":90}
{"cmd":"goto","unit":1,"waypoints":[{"x":1,"y":2}]}
{"cmd":"set_radar","unit":1,"active":true}
{"cmd":"set_sonar","unit":1,"active":false}
{"cmd":"set_weapons_free","unit":1,"free":true}
{"cmd":"set_engagement_policy","unit":1,"air_only":true,"max_in_flight":2}
{"cmd":"assign_target","unit":1,"contact":1}
{"cmd":"fire","unit":1,"contact":1,"weapon":"an_observed_weapon_id","rounds":2}
{"cmd":"launch_aircraft","unit":1,"class":"a_ready_class","count":2,
 "mission":{"mission":"cap","station":{"x":1,"y":2},"radius_nm":20,"follow":true}}
Other mission variants: {"mission":"asw","area":{"x":1,"y":2},"radius_nm":12},
{"mission":"recon","point":{"x":1,"y":2}}, {"mission":"strike","contact":1},
{"mission":"rtb"}. Launch profile may be null or omitted; do not assume instant
launch or recovery. retask_aircraft uses unit,mission,manual:false.
return_to_base uses unit. clear_target uses unit. damage_control uses unit,action.
Do not issue set_unit_automation; this experiment has a fixed controller owner.
"""

INSTRUCTIONS = """You are the sole high-level commander of one side in a fictional
naval simulation. Use only the supplied own observation and public definitions.
No tools, files, internet, code execution or external information. The world
continues while you reason, and prior orders and basic self-defence continue.
Maximize the scenario's final weighted confirmed score relative to the opponent,
while protecting your force and preserving scarce weapons, aircraft and endurance.
This is a simulation with stated RTS abstractions, not real weapon validation.
Reply only with the schema. commands_json is a JSON-encoded array of at most 16
commands; [] explicitly reviews and continues standing tasks. Give a short public
brief, not a reasoning trace. observation_tick must match the input tick.
All responses also specify future wake_conditions. These are OR clauses of AND
atoms. Use only the documented numeric scheduler fields and operators; absolute
thresholds refer to this observation. E.g. contact_revision > current revision.
An empty condition list is allowed. There is no implicit time-based fallback in
an event-only policy. Existing conditions remain active until a valid response
replaces them. Costs of initialization, retries and condition generation count.
""" + COMMAND_REFERENCE

SCHEMA = {
    'type': 'object', 'additionalProperties': False,
    'required': ['observation_tick', 'commands_json', 'wake_conditions', 'brief'],
    'properties': {
        'observation_tick': {'type': 'integer'},
        'commands_json': {'type': 'string'},
        'brief': {'type': 'string'},
        'wake_conditions': {'type': 'array', 'maxItems': 4, 'items': {
            'type': 'object', 'additionalProperties': False, 'required': ['all'],
            'properties': {'all': {'type': 'array', 'minItems': 1, 'maxItems': 3, 'items': {
                'type': 'object', 'additionalProperties': False,
                'required': ['field', 'op', 'value'], 'properties': {
                    'field': {'type': 'string', 'enum': list(FIELDS)},
                    'op': {'type': 'string', 'enum': ['eq', 'ne', 'gt', 'ge', 'lt', 'le']},
                    'value': {'type': 'number'},
                }}}}}},
    },
}


def validate_answer(answer, model_input):
    from naval_scheduler import validate_conditions
    if set(answer) != set(SCHEMA['required']):
        raise ValueError('response fields')
    obs = model_input['observation']
    if answer['observation_tick'] != obs['tick']:
        raise ValueError('observation identity')
    commands = json.loads(answer['commands_json'])
    def finite(value):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError('non-finite command value')
        if isinstance(value, dict):
            for x in value.values():finite(x)
        elif isinstance(value, list):
            for x in value:finite(x)
    finite(commands)
    if not isinstance(commands, list) or len(commands) > 16:
        raise ValueError('command count')
    ids = {u['id'] for u in obs['units']}
    contacts = {c['id'] for c in obs['contacts']}
    for c in commands:
        if (not isinstance(c, dict) or type(c.get('unit')) is not int or
                c.get('unit') not in ids or c.get('cmd') == 'set_unit_automation'):
            raise ValueError('command ownership')
        if 'contact' in c and c['contact'] not in contacts:
            raise ValueError('unobserved contact')
        mission = c.get('mission')
        if isinstance(mission, dict) and 'contact' in mission and mission['contact'] not in contacts:
            raise ValueError('unobserved mission contact')
    validate_conditions(answer['wake_conditions'])
    if not isinstance(answer['brief'], str):
        raise ValueError('brief')
