"""Windows CLI regression: invoke the built offline authoring executable, never Cargo."""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

EXE = Path(sys.argv.pop(1)).resolve()
REPO = Path(__file__).resolve().parents[2]


class AuthoringWorkflow(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="card-author-")
        self.root = Path(self.temp.name)
        self.bulk = self.root / "source.jsonl.gz"
        self.target = {
            "name": "Authoring Trial", "oracle_id": "trial",
            "layout": "normal", "mana_cost": "{3}{U}",
            "type_line": "Sorcery", "oracle_text": "Draw three cards.",
            "rulings_uri": "https://example.invalid/rulings",
        }
        source = dict(self.target, name="Divination", oracle_id="divination",
                      mana_cost="{2}{U}", oracle_text="Draw two cards.")
        with gzip.open(self.bulk, "wt", encoding="utf-8") as out:
            for card in (source, self.target):
                out.write(json.dumps(card) + "\n")
        self.meta = Path(str(self.bulk) + ".meta.json")
        self.meta.write_text(json.dumps({"type": "oracle_cards", "sha256": hashlib.sha256(self.bulk.read_bytes()).hexdigest()}))

    def tearDown(self):
        self.temp.cleanup()

    def call(self, *args, ok=True):
        result = subprocess.run([str(EXE), *map(str, args)], cwd=REPO,
                                capture_output=True, text=True, encoding="utf-8")
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
            return json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0, result.stdout)
        return result.stderr

    def test_clone_never_copies_approval_or_overwrites_and_reports_differences(self):
        out = self.root / "draft"
        self.call("clone", "--from", "divination", "--name", "Authoring Trial",
                  "--bulk", self.bulk, "--out", out)
        draft = (out / "authoring_trial.draft.ron").read_text()
        self.assertIn("__mechanics_unresolved", draft)
        review = json.loads((out / "authoring_trial.json").read_text())
        self.assertFalse(review["complete_definition_review_confirmed"])
        self.assertEqual(review["semantic_fixtures"], [])
        diff = json.loads((out / "differences.json").read_text())
        self.assertIn("oracle_text", [row["field"] for row in diff["source_differences"]])
        self.call("clone", "--from", "divination", "--name", "Authoring Trial",
                  "--bulk", self.bulk, "--out", out, ok=False)
        self.assertEqual((out / "authoring_trial.draft.ron").read_text(), draft)

    def test_clone_rejects_implemented_target_and_embedded_output(self):
        self.call("clone", "--from", "divination", "--name", "Divination",
                  "--bulk", self.bulk, "--out", self.root / "bad", ok=False)
        self.call("clone", "--from", "divination", "--name", "Authoring Trial",
                  "--bulk", self.bulk, "--out", REPO / "tricerules/tricerules-cards/data/forbidden-draft", ok=False)
        self.assertFalse((REPO / "tricerules/tricerules-cards/data/forbidden-draft").exists())

    def test_source_integrity_and_typed_search(self):
        result = self.call("search", "--like", "divination", "--limit", "3")
        self.assertTrue(result["advisory_only"])
        self.assertTrue(all("path" in row and "features" in row for row in result["matches"]))
        self.meta.write_text('{"type":"oracle_cards","sha256":"wrong"}')
        error = self.call("search", "--name", "Authoring Trial", "--bulk", self.bulk, ok=False)
        self.assertIn("SHA mismatch", error)

    def test_queue_survives_unrelated_changes_and_rejects_changed_evidence(self):
        dep = self.root / "rulings.txt"
        dep.write_text("reviewed")
        entry = self.root / "queue.json"
        self.call("queue-save", "--name", "Authoring Trial", "--note", "reviewed analogy",
                  "--depends", dep, "--out", entry)
        (self.root / "unrelated.txt").write_text("changed HEAD is irrelevant")
        self.assertTrue(self.call("queue-check", "--entry", entry)["fresh"])
        dep.write_text("revised")
        self.assertFalse(self.call("queue-check", "--entry", entry)["fresh"])
        dep.unlink()
        self.assertFalse(self.call("queue-check", "--entry", entry)["fresh"])


if __name__ == "__main__":
    unittest.main()
