"""Negative contract tests for the immutable Git-tree scope checker.

All data are synthetic dictionaries; no repository, network or files touched.
"""
from __future__ import annotations

import unittest

from cross_pr_attestor import Hold, added_only, disjoint_groups


A = ("100644", "blob", "a" * 40)
B = ("100644", "blob", "b" * 40)


class CrossPrAttestationContract(unittest.TestCase):
    def setUp(self):
        self.base = {"src/frozen.rs": A, "qa/phase6/old-test.mjs": B}
        self.added = {"qa/phase6/runtime/new.py": A}

    def valid(self):
        result = dict(self.base)
        result.update(self.added)
        return result

    def reject(self, target, allow=None):
        with self.assertRaises(Hold):
            added_only(self.base, target, frozenset(self.added) if allow is None else allow)

    def test_01_exact_addition_preserves_all_frozen_files(self):
        added_only(self.base, self.valid(), frozenset(self.added))

    def test_02_original_blob_rewrite_fails(self):
        target = self.valid()
        target["src/frozen.rs"] = B
        self.reject(target)

    def test_03_original_mode_rewrite_fails(self):
        target = self.valid()
        target["src/frozen.rs"] = ("100755", "blob", A[2])
        self.reject(target)

    def test_04_missing_original_fails(self):
        target = self.valid()
        target.pop("src/frozen.rs")
        self.reject(target)

    def test_05_unknown_added_workflow_fails(self):
        target = self.valid()
        target[".github/workflows/rogue.yml"] = A
        self.reject(target)

    def test_06_missing_expected_addition_fails(self):
        self.reject(self.base)

    def test_07_symlink_mode_fails(self):
        target = self.valid()
        target["qa/phase6/runtime/new.py"] = ("120000", "blob", A[2])
        self.reject(target)

    def test_08_executable_added_mode_fails(self):
        target = self.valid()
        target["qa/phase6/runtime/new.py"] = ("100755", "blob", A[2])
        self.reject(target)

    def test_09_submodule_mode_fails(self):
        target = self.valid()
        target["qa/phase6/runtime/new.py"] = ("160000", "commit", A[2])
        self.reject(target)

    def test_10_non_git_object_hash_fails(self):
        target = self.valid()
        target["qa/phase6/runtime/new.py"] = ("100644", "blob", "FAKE")
        self.reject(target)

    def test_11_cross_agent_file_collision_fails(self):
        with self.assertRaises(Hold):
            disjoint_groups({"ai09": frozenset({"one", "two"}),
                             "ai10": frozenset({"two", "three"})})

    def test_12_empty_agent_group_fails(self):
        with self.assertRaises(Hold):
            disjoint_groups({"ai09": frozenset()})

    def test_13_disjoint_allowlists_pass(self):
        disjoint_groups({"ai09": frozenset({"one"}),
                         "ai10_policy": frozenset({"two"}),
                         "ai10_session": frozenset({"three"})})

    def test_14_existing_fixture_test_change_fails(self):
        target = self.valid()
        target["qa/phase6/old-test.mjs"] = ("100644", "blob", "c" * 40)
        self.reject(target)

    def test_15_unlisted_missing_payload_rejected(self):
        target = self.valid()
        target["docs/hiding.md"] = B
        self.reject(target, allow=frozenset(self.added))


if __name__ == "__main__":
    unittest.main()
