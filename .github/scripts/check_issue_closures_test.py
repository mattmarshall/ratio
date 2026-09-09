#!/usr/bin/env python3
"""Exercise the real parser and event/commit boundary, including the old failure."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).parent))
from check_issue_closures import check_documents

SCRIPT = Path(__file__).with_name("check_issue_closures.py").resolve()


class IssueClosureWording(unittest.TestCase):
    def errors(self, text):
        return check_documents([("body", text, True)], "mattmarshall/ratio")

    def test_neutral_references_leave_partial_work_open(self):
        self.assertEqual([], self.errors("Related: #22\nRemaining work: #150 — registration.\nA closing checklist is not a directive."))

    def test_each_github_keyword_can_deliberately_complete_an_issue(self):
        for keyword in ("close", "closes", "closed", "fix", "fixes", "fixed", "resolve", "resolves", "resolved"):
            for reference in ("#269", "mattmarshall/ratio#269", "https://github.com/mattmarshall/ratio/issues/269"):
                with self.subTest(keyword=keyword, reference=reference):
                    self.assertEqual([], self.errors(f"Acceptance and verification are complete.\n\n{keyword.upper()}: {reference}"))

    def test_the_previous_negated_guidance_is_refused(self):
        for text in ("This does not close #22.", "This doesn't fix #150.", "Do not resolve #161.", "This does not\nclose #22.", "This does not close\n#22."):
            with self.subTest(text=text):
                self.assertTrue(any("standalone completion line" in error for error in self.errors(text)))

    def test_inline_completion_and_partial_completion_are_refused(self):
        for text in ("Partially fixes #22", "Closes #22 except registration", "Once approved, closes #22", "Closes #22, closes #150", "Closes #22\nexcept registration"):
            with self.subTest(text=text):
                self.assertTrue(self.errors(text))

    def test_quoted_examples_are_not_completion_evidence(self):
        for text in ("> Closes #22", "`Closes #22`", "```\nCloses #22\n```", "~~~text\nCloses #22\n~~~", "<!--\nCloses #22\n-->"):
            with self.subTest(text=text):
                self.assertTrue(self.errors(text))

    def test_a_related_issue_cannot_also_be_completed(self):
        for reference in ("#22", "mattmarshall/ratio#22", "https://github.com/mattmarshall/ratio/issues/22"):
            with self.subTest(reference=reference):
                errors = self.errors(f"Closes #22\nRemaining work: {reference}")
                self.assertTrue(any("both completion" in e for e in errors))

    def test_a_partial_parent_does_not_prevent_completing_its_child(self):
        self.assertEqual([], self.errors("Remaining work: #22\nCloses #269"))
        self.assertEqual([], self.errors("Related: other/repo#269\nCloses #269"))

    def test_a_commit_cannot_close_an_issue_the_pr_leaves_open(self):
        errors = check_documents([("body", "Remaining work: #22", True), ("commit", "Closes #22", True)], "mattmarshall/ratio")
        self.assertTrue(any("both completion" in e for e in errors))

    def test_a_title_cannot_supply_a_closing_directive(self):
        self.assertTrue(check_documents([("title", "Fix #22", False)], "mattmarshall/ratio"))

    def test_the_workflow_event_checks_commit_messages_as_well_as_body_edits(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True, text=True).stdout.strip()
            git("init", "-q")
            git("config", "commit.gpgsign", "false")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")
            git("commit", "--allow-empty", "-qm", "Baseline")
            base = git("rev-parse", "HEAD")
            git("commit", "--allow-empty", "-qm", "This does not close #22")
            event = {"repository": {"full_name": "mattmarshall/ratio"}, "pull_request": {"title": "Improve activation", "body": "Related: #22", "base": {"sha": base}, "head": {"sha": git("rev-parse", "HEAD")}}}
            path = root / "event.json"
            def check():
                path.write_text(json.dumps(event))
                return subprocess.run([sys.executable, str(SCRIPT), "--event", str(path)], cwd=root, capture_output=True, text=True)
            result = check()
            self.assertEqual(1, result.returncode)
            self.assertIn("commit 1:1", result.stderr)
            git("commit", "--amend", "--allow-empty", "-qm", "Improve activation\n\nRelated: #22")
            event["pull_request"]["head"]["sha"] = git("rev-parse", "HEAD")
            self.assertEqual(0, check().returncode)
            event["pull_request"]["body"] = "Partially fixes #22"
            result = check()
            self.assertEqual(1, result.returncode)
            self.assertIn("PR body:1", result.stderr)


if __name__ == "__main__":
    unittest.main()
