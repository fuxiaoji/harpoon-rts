import copy
import unittest
from naval_inputs import PublicFeatures, project_input, FIELDS,validate_answer
import json

def observation():
    return {'tick': 0, 'your_side': 'Blue', 'units': [
        {'id': 1, 'side': 'Blue', 'pos': {'x': 1, 'y': 2}, 'mission': None}],
        'contacts': [{'id': 9, 'category': 'Ship', 'class_name': None, 'stale': False,
                      'estimate': {'x': 50, 'y': 50}, 'bearing_only': True}],
        'victory': {'own_losses': 0}, 'scenario_description': 'private labels are omitted'}

class TestPublicInputs(unittest.TestCase):
    def test_motion_does_not_change_event_identity(self):
        obs=observation(); tracker=PublicFeatures(); a=tracker.update(obs)
        obs['tick']=10; obs['contacts'][0]['estimate']['x']=51
        b=tracker.update(obs)
        self.assertEqual(a['contact_revision'],b['contact_revision'])
        obs['contacts'][0]['stale']=True
        self.assertEqual(tracker.update(obs)['contact_revision'],1)

    def test_same_count_replaced_incoming_is_detected(self):
        obs=observation();obs['contacts'][0]['category']='Missile'
        tracker=PublicFeatures();tracker.update(obs)
        obs['contacts'][0]['id']=10
        f=tracker.update(obs)
        self.assertEqual(f['incoming_count'],1)
        self.assertEqual(f['incoming_revision'],1)

    def test_restore_and_projection(self):
        obs=observation(); t=PublicFeatures();f=t.update(obs)
        other=PublicFeatures(t.state)
        obs['tick']=50
        self.assertEqual(t.update(obs),other.update(obs))
        x=project_input(obs,t.update(obs),{})
        self.assertNotIn('scenario_description',x['observation'])
        self.assertEqual(set(x['scheduler_features']),set(FIELDS))
        self.assertNotIn('world',x)
        bad=copy.deepcopy(obs);bad['units'][0]['side']='Red'
        with self.assertRaises(ValueError):project_input(bad,f,{})

    def test_nonfinite_command_and_wrong_observation_are_rejected(self):
        obs=observation();f=PublicFeatures().update(obs)
        inp=project_input(obs,f,{})
        reply={'observation_tick':0,'commands_json':'[]','wake_conditions':[],'brief':'hold'}
        validate_answer(reply,inp)
        reply['commands_json']='[{"cmd":"set_speed","unit":1,"speed_kts":1e999}]'
        with self.assertRaises(ValueError):validate_answer(reply,inp)
        reply['commands_json']='[]';reply['observation_tick']=1
        with self.assertRaises(ValueError):validate_answer(reply,inp)

if __name__=='__main__':unittest.main()
