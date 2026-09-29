"""Readiness and frozen-review boundary regressions; no production card changes."""
import importlib.util
import json
import shutil
from pathlib import Path
import subprocess
import tempfile
import unittest
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("authoring_batch", ROOT / "scripts/authoring-batch.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BatchTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.packet = {"cards": [{"id": "trial", "name": "Trial", "oracle_id": "oracle",
                                   "oracle_lines": [{"face": 0, "line": 1, "text": "Draw two cards."}]}]}
        self.card = {"id": "trial", "route": "reuse_only", "differences": "Only quantity differs",
                     "rulings": "rulings.json", "clauses": [{"face": 0, "line": 1, "row_indices": [0], "tests": []}],
                     "interactions": {key: "N/A: simple spell" for key in module.INTERACTIONS}}
        (self.root / "rulings.json").write_text('{"object":"list","data":[]}')
        self.rows = [{"family": "draw", "card": "trial"}]

    def tearDown(self):
        self.temp.cleanup()

    def test_readiness_requires_every_clause_and_correct_card_evidence(self):
        module.validate_assessments(self.packet, [self.card], self.rows, self.root)
        self.card["clauses"] = []
        with self.assertRaises(ValueError):
            module.validate_assessments(self.packet, [self.card], self.rows, self.root)

    def test_readiness_rejects_duplicate_clause_mappings(self):
        self.card["clauses"] *= 2
        with self.assertRaises(ValueError):
            module.validate_assessments(self.packet, [self.card], self.rows, self.root)
        self.card["clauses"] = [{"face": 0, "line": 1, "row_indices": [0], "tests": []}]
        self.rows[0]["card"] = "other"
        with self.assertRaises(ValueError):
            module.validate_assessments(self.packet, [self.card], self.rows, self.root)

    def test_readiness_rejects_blockers_missing_interactions_and_duplicate_clauses(self):
        self.card["route"] = "engine_capability"
        with self.assertRaises(ValueError):
            module.validate_assessments(self.packet, [self.card], self.rows, self.root)
        self.card["route"] = "reuse_only"
        self.card["interactions"].pop("simultaneous")
        with self.assertRaises(ValueError):
            module.validate_assessments(self.packet, [self.card], self.rows, self.root)

    def test_freeze_captures_untracked_and_deleted_paths_and_detects_later_edits(self):
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.email", "test@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(self.root), "config", "user.name", "Test"], check=True)
        (self.root / "tracked.txt").write_text("base\n")
        subprocess.run(["git", "-C", str(self.root), "add", "tracked.txt"], check=True)
        subprocess.run(["git", "-C", str(self.root), "commit", "--quiet", "-m", "base"], check=True)
        (self.root / "tracked.txt").unlink()
        (self.root / "new.txt").write_text("untracked\n")
        out = self.root / "review"
        module.freeze(self.root, ["tracked.txt", "new.txt"], [], out)
        manifest = json.loads((out / "manifest.json").read_text())
        self.assertTrue(module.fresh(manifest)["fresh"])
        patch = (out / "patch.diff").read_text()
        self.assertIn("deleted file", patch)
        self.assertIn("new file", patch)
        (self.root / "new.txt").write_text("different\n")
        self.assertFalse(module.fresh(manifest)["fresh"])
        with self.assertRaises(ValueError):
            module.freeze(self.root, ["../outside"], [], self.root / "bad")

    def test_failed_phase_is_preserved_and_command_exit_is_returned(self):
        import sys
        result = module.run_phase(self.root, self.root / "phases", "focused", [sys.executable, "-c", "raise SystemExit(7)"])
        self.assertEqual(result["exit_code"], 7)
        self.assertGreaterEqual(result["elapsed_seconds"], 0)
        self.assertEqual(len(list((self.root / "phases").glob("*.json"))), 1)

    def test_map_check_rejects_missing_pointers_and_uncovered_source_lines(self):
        self.card["review_map"] = "map.json"
        mapping = {"format_version": 1, "oracle_id": "oracle", "spans": [
            {"face_id": "trial", "start_line": 1, "end_line": 1, "typed_paths": ["/spell_effect/0"]}]}
        path = self.root / "map.json"
        draft = {"id": "trial", "name": "Trial", "faces": ["trial"], "typed": {"spell_effect": ["Draw"]}}
        module.write(path, mapping)
        module.validate_maps(self.packet, [self.card], [draft], self.root)
        for invalid in ("-1", "01", "+0", "1", "١"):
            mapping["spans"][0]["typed_paths"] = [f"/spell_effect/{invalid}"]
            module.write(path, mapping)
            with self.assertRaises(ValueError, msg=invalid):
                module.validate_maps(self.packet, [self.card], [draft], self.root)
        mapping["spans"][0]["typed_paths"] = ["/missing"]
        module.write(path, mapping)
        with self.assertRaises(ValueError):
            module.validate_maps(self.packet, [self.card], [draft], self.root)
        mapping["spans"] = []
        module.write(path, mapping)
        with self.assertRaises(ValueError):
            module.validate_maps(self.packet, [self.card], [draft], self.root)

    def test_disk_preflight_reports_insufficient_capacity(self):
        result = module.doctor(self.root, 1e12, inspect_git=False)
        self.assertFalse(result["sufficient_disk"])
        self.assertGreater(result["free_gib"], 0)

    def test_native_flags_and_failure_exit_survive_both_powershell_wrappers(self):
        for shell in ("powershell.exe", "pwsh.exe"):
            executable = shutil.which(shell)
            if not executable:
                continue
            with self.subTest(shell=shell):
                result = subprocess.run([executable, "-NoProfile", "-File", str(ROOT / "scripts/authoring-batch.ps1"),
                                         "phase", "--name", "wrapper", "--out", str(self.root / (shell + " space")),
                                         "--", sys.executable, "-c", "raise SystemExit(7)"],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 7, result.stderr)
                self.assertEqual(json.loads(result.stdout)["exit_code"], 7)


if __name__ == "__main__":
    unittest.main()
