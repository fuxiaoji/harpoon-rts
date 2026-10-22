import unittest
import resource_study as r

class PublicPlannerTests(unittest.TestCase):
    def test_combination_uses_shared_budget_and_task_exclusion(self):
        case={"tasks":[{"id":0,"kits":2},{"id":1,"kits":2}],"shared_kits":2}
        catalog=[]
        for u in (1,2,3):
            for mask,score in ((0,0),(1,8),(2,6)):
                catalog.append({"unit":u,"mask":mask,"score":score,"return_tick":10,"feasible":True,"task_ids":[mask-1] if mask else []})
        plan,score=r.solve(case,catalog)
        self.assertEqual(score,8)
        self.assertEqual(sum(len(x["task_ids"]) for x in plan["routes"]),1)

    def test_infeasible_high_reward_cannot_enter_reference(self):
        case={"tasks":[{"id":0,"kits":1}],"shared_kits":1}
        catalog=[{"unit":u,"mask":0,"score":0,"return_tick":0,"feasible":True,"task_ids":[]} for u in (1,2,3)]
        catalog.append({"unit":1,"mask":1,"score":100,"return_tick":1,"feasible":False,"task_ids":[0]})
        self.assertEqual(r.solve(case,catalog)[1],0)

    def test_semantic_mistake_is_scored_not_deleted(self):
        c=r.generate(11,"complex")
        plan={"routes":[{"unit":1,"task_ids":[1]},{"unit":2,"task_ids":[]},{"unit":3,"task_ids":[]}],"brief":"Invalid capability/predecessor still valid action."}
        r.validate(plan,c)
        plan["routes"][1]["task_ids"]=[1]
        with self.assertRaises(ValueError):r.validate(plan,c)

    def test_generation_does_not_depend_on_budget_or_urgency(self):
        a=r.generate(23,"complex");b=r.generate(23,"complex")
        self.assertEqual(a,b)
        self.assertEqual(len(a["tasks"]),9)
        self.assertEqual([t["predecessor"] for t in a["tasks"] if t["predecessor"] is not None],[0,2])

if __name__=="__main__":unittest.main()
