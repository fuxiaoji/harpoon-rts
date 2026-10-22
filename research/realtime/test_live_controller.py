import copy
import unittest
import live_controller as live


class LiveTests(unittest.TestCase):
    def test_new_revision_rearms_true_condition_only_in_keyed_mode(self):
        condition=[{"all":[{"field":"task_mismatch","op":"eq","value":1}]}]
        states=[{"task_mismatch":v,"task_revision":r} for v,r in ((0,0),(1,1),(1,2))]
        legacy=live.Watch(condition)
        keyed=live.Watch(condition,True)
        self.assertEqual([legacy.poll(s) for s in states],[False,True,False])
        self.assertEqual([keyed.poll(s) for s in states],[False,True,True])

    def test_repeated_notices_do_not_rearm_task_condition(self):
        watch=live.Watch([{"all":[{"field":"task_mismatch","op":"eq","value":1}]}],True)
        self.assertEqual([watch.poll({"task_mismatch":1,"task_revision":1,"notice_seq":n}) for n in range(3)],
                         [True,False,False])
    def test_one_event_per_false_to_true_edge(self):
        watch=live.Watch([{"all":[{"field":"task_mismatch","op":"eq","value":1}]}])
        self.assertEqual([watch.poll({"task_mismatch":v}) for v in (0,1,1,0,1)],[False,True,False,False,True])

    def test_conditions_use_and_within_or_between(self):
        watch=live.Watch([{"all":[{"field":"tick","op":"ge","value":5},
                                 {"field":"task_revision","op":"eq","value":1}]},
                          {"all":[{"field":"tick","op":"ge","value":10}]}])
        self.assertFalse(watch.poll({"tick":6,"task_revision":0}))
        self.assertTrue(watch.poll({"tick":7,"task_revision":1}))
        self.assertTrue(watch.poll({"tick":10,"task_revision":1}))

    def fixture(self):
        state={"observation":{"tick":20,"units":[{"id":1,"pos":{"x":0,"y":0}}]},
               "public_task":{"revision":1,"deadline_tick":50,"goal":{"x":3,"y":4}},"standing_revision":0}
        answer={"action":"goto","task_revision":1,"unit":1,"waypoint":{"x":3,"y":4},"brief":"Navigate.",
                "wake_conditions":[{"all":[{"field":"task_mismatch","op":"eq","value":1}]}]}
        return state,answer

    def test_features_are_derived_from_public_snapshot(self):
        state,_=self.fixture()
        f=live.features(state,5)
        self.assertEqual((f["distance_nm"],f["deadline_remaining_s"],f["plan_age_s"],f["task_mismatch"]),(5,30,15,1))

    def test_unknown_fields_and_nonfinite_thresholds_rejected(self):
        state,answer=self.fixture()
        for field,value in (("enemy_truth",1),("tick",float("nan"))):
            wrong=copy.deepcopy(answer)
            wrong["wake_conditions"][0]["all"][0].update(field=field,value=value)
            with self.assertRaises(ValueError):live.validate(wrong,state)

    def test_valid_condition_and_empty_condition_set(self):
        state,answer=self.fixture()
        live.validate(answer,state)
        answer["wake_conditions"]=[]
        live.validate(answer,state)

    def test_stale_action_is_not_accepted_with_valid_watch(self):
        state,answer=self.fixture()
        answer["task_revision"]=0
        with self.assertRaises(ValueError):live.validate(answer,state)


if __name__=="__main__":unittest.main()
