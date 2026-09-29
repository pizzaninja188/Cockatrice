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

    def test_prepare_resolves_batch_once_and_preserves_unassessed_boundary(self):
        names = self.root / "names.txt"
        names.write_text("Authoring Trial\nDivination\n", encoding="utf-8")
        corpus = self.root / "corpus.tsv"
        corpus.write_text("Authoring Trial\ttrial\tTest\twhole-card\tAuthoring Trial\nDivination\tdivination\tTest\twhole-card\tDivination\n")
        out = self.root / "packet"
        self.call("prepare", "--names", names, "--bulk", self.bulk,
                  "--corpus", corpus, "--out", out, "--limit", "2")
        packet = json.loads((out / "packet.json").read_text())
        self.assertEqual(len(packet["cards"]), 2)
        trial, registered = packet["cards"]
        self.assertFalse(trial["registered"])
        self.assertTrue(registered["registered"])
        self.assertEqual(trial["route"], "unassessed")
        self.assertFalse(packet["semantic_approval"])
        self.assertEqual(trial["oracle_lines"][0]["text"], "Draw three cards.")
        self.assertIn("oracle_text", [d["field"] for d in trial["analogues"][0]["differences"]])
        self.assertTrue(self.call("queue-check", "--entry", out / "dependencies.json")["fresh"])
        corpus.write_text("changed corpus")
        self.assertFalse(self.call("queue-check", "--entry", out / "dependencies.json")["fresh"])
        self.call("prepare", "--names", names, "--bulk", self.bulk, "--out", out, ok=False)

    def test_prepare_rejects_scope_mismatch_and_duplicate_names_before_output(self):
        names = self.root / "names.txt"
        names.write_text("Authoring Trial\nAuthoring Trial\n")
        out = self.root / "bad"
        self.call("prepare", "--names", names, "--bulk", self.bulk, "--out", out, ok=False)
        self.assertFalse(out.exists())
        names.write_text("Authoring Trial\n")
        corpus = self.root / "corpus.tsv"
        corpus.write_text("Authoring Trial\twrong-id\tTest\twhole-card\tAuthoring Trial\n")
        self.call("prepare", "--names", names, "--bulk", self.bulk,
                  "--corpus", corpus, "--out", out, ok=False)
        self.assertFalse(out.exists())

    def test_validate_batch_rejects_unexercised_drafts_and_unknown_row_fields(self):
        draft = self.root / "trial.ron"
        draft.write_text('(id:"authoring_trial",name:"Authoring Trial",face_id:"authoring_trial",mana_cost:"{3}{U}",types:["Sorcery"],spell_effect:[Draw(count:3)])')
        batch = self.root / "batch.json"
        row = dict(family="draw", card="authoring_trial", mana=[0,1,0,0,0,3],
                   surface="spell", recipient=0, count=3, food=0)
        batch.write_text(json.dumps(dict(drafts=["trial.ron"], rows=[row])))
        result = self.call("validate-batch", "--batch", batch)
        self.assertFalse(result["semantic_approval"])
        self.assertEqual(result["drafts"][0]["id"], "authoring_trial")
        row["card"] = "divination"
        batch.write_text(json.dumps(dict(drafts=["trial.ron"], rows=[row])))
        self.call("validate-batch", "--batch", batch, ok=False)
        row["card"] = "authoring_trial"
        row["guessed"] = True
        batch.write_text(json.dumps(dict(drafts=["trial.ron"], rows=[row])))
        self.call("validate-batch", "--batch", batch, ok=False)

    def test_preflight_integration_binds_maps_rows_source_and_freshness(self):
        names = self.root / "names.txt"
        names.write_text("Authoring Trial\n")
        prepared = self.root / "prepared"
        self.call("prepare", "--names", names, "--bulk", self.bulk, "--out", prepared)
        (self.root / "trial.ron").write_text('(id:"authoring_trial",name:"Authoring Trial",face_id:"authoring_trial",mana_cost:"{3}{U}",types:["Sorcery"],spell_effect:[Draw(count:3)])')
        rows = [dict(family="draw", card="authoring_trial", mana=[0,1,0,0,0,3], surface="spell", recipient=0, count=3, food=0)]
        (self.root / "drafts.json").write_text(json.dumps(dict(drafts=["trial.ron"], rows=rows)))
        (self.root / "rulings.json").write_text('{"object":"list","data":[]}')
        mapping = dict(format_version=1, oracle_id="trial", spans=[dict(face_id="authoring_trial", start_line=1, end_line=1, typed_paths=["/faces/0/spell_effect/0"])])
        (self.root / "map.json").write_text(json.dumps(mapping))
        card = dict(id="authoring_trial", route="reuse_only", differences="Divination quantity differs",
                    rulings="rulings.json", review_map="map.json", clauses=[dict(face=0, line=1, row_indices=[0], tests=[])],
                    interactions={k: "Checked: unchanged analogue" for k in ("timing","simultaneous","identity","choices","costs","targets","tokens","presentation","client")})
        assessment = dict(version=1, packet="prepared/packet.json", freshness="prepared/dependencies.json", draft_batch="drafts.json", cards=[card])
        manifest = self.root / "assessment.json"
        manifest.write_text(json.dumps(assessment))
        report = self.root / "preflight.json"
        script = REPO / "scripts/authoring-batch.py"
        def run(*args, ok=True):
            result = subprocess.run([sys.executable, str(script), *map(str,args)], capture_output=True, text=True)
            self.assertEqual(result.returncode == 0, ok, result.stderr)
            return json.loads(result.stdout) if ok else None
        result = run("preflight", "--manifest", manifest, "--exe", EXE, "--out", report)
        self.assertFalse(result["semantic_approval"])
        self.assertTrue(run("check", "--manifest", report)["fresh"])
        (self.root / "trial.ron").write_text("changed draft")
        run("check", "--manifest", report, ok=False)


if __name__ == "__main__":
    unittest.main()
