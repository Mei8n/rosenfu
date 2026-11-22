import copy
import unittest

from guard import BASE, BASE_REF, HEAD_REF, LABEL, PATHS, REPO, check_checkout, check_event

SHA="a"*40
ENV={"GITHUB_EVENT_NAME":"pull_request","GITHUB_RUN_ATTEMPT":"1","GITHUB_REPOSITORY":REPO}
EVENT={"action":"labeled","number":285,"label":{"name":LABEL},"pull_request":{
    "number":285,"draft":True,"state":"open",
    "head":{"ref":HEAD_REF,"sha":SHA,"repo":{"full_name":REPO}},
    "base":{"ref":BASE_REF,"repo":{"full_name":REPO}},
}}


class Scope(unittest.TestCase):
    def test_valid_event_and_exact_product_child(self):
        self.assertEqual(check_event(EVENT,ENV),SHA)
        check_checkout(SHA,[SHA,BASE],sorted(PATHS))

    def test_untrusted_text_is_not_passed_through_as_sha(self):
        for sha in ['";touch /tmp/pwn#','$(echo untrusted)', SHA+'\n', 'b'*41, None, 9]:
            bad=copy.deepcopy(EVENT); bad['pull_request']['head']['sha']=sha
            with self.assertRaises(ValueError):
                check_event(bad,ENV)

    def test_fork_other_head_label_wrong_ref_closed_ready_rerun_and_merge_ref(self):
        for field,value in [('draft',False),('draft','true'),('state','closed'),('number',284)]:
            bad=copy.deepcopy(EVENT);bad['pull_request'][field]=value
            with self.assertRaises(ValueError):check_event(bad,ENV)
        for side in ['head','base']:
            for target in [{'ref':'main'}, {'repo':{'full_name':'someone/fork'}}, {'ref':'${untrusted}'}]:
                bad=copy.deepcopy(EVENT);bad['pull_request'][side].update(target)
                with self.assertRaises(ValueError):check_event(bad,ENV)
        for action,label in [('synchronize',LABEL),('labeled',LABEL+'-2'),('unlabeled',LABEL)]:
            bad=copy.deepcopy(EVENT);bad['action']=action;bad['label']['name']=label
            with self.assertRaises(ValueError):check_event(bad,ENV)
        for override in [{'GITHUB_RUN_ATTEMPT':'2'}, {'GITHUB_EVENT_NAME':'pull_request_target'}, {'GITHUB_REPOSITORY':'unrelated/repo'}]:
            with self.assertRaises(ValueError):check_event(EVENT,ENV|override)

    def test_product_changes_or_unreviewed_transition_are_refused(self):
        for parents,changes in [([SHA,'b'*40],sorted(PATHS)),([SHA,BASE,'b'*40],sorted(PATHS)),
                ([SHA,BASE],list(PATHS)+['src/App.tsx']),
                ([SHA,BASE],[]),([SHA,BASE],list(PATHS)+['scripts/native-desktop-smoke/evil\nrun.sh'])]:
            with self.assertRaises(ValueError):
                check_checkout(SHA,parents,changes)


if __name__=='__main__': unittest.main()
