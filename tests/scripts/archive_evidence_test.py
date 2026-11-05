import json
import shutil
import subprocess
import sys
import unittest
import uuid
import zipfile
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "archive-evidence.py"


class ArchiveEvidenceTest(unittest.TestCase):
    def setUp(self):
        self.root = SCRIPT.parents[1] / "build" / f"evidence-test-{uuid.uuid4()}"
        self.root.mkdir(parents=True)
        self.addCleanup(shutil.rmtree, self.root)
        self.script = self.root / "scripts" / SCRIPT.name
        self.script.parent.mkdir()
        shutil.copyfile(SCRIPT, self.script)
        self.evidence = self.root / "build" / "verification-logs"
        self.evidence.mkdir(parents=True)

    def invoke(self, *args, success=True):
        result = subprocess.run(
            [sys.executable, str(self.script), *map(str, args)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        return result

    def test_round_trip_and_no_overwrite(self):
        run = self.evidence / "verify-closed run"
        (run / "empty").mkdir(parents=True)
        summary = b'{"Status":"Fail","CompletedAt":"2026-11-05T10:00:00Z"}'
        frames = b"\x00\x01\x02sample frame\n" * 4096
        (run / "summary.json").write_bytes(summary)
        (run / "frames.pb").write_bytes(frames)
        archive = run.with_suffix(".zip")

        self.invoke("pack", run, "--remove-source")
        self.assertFalse(run.exists())
        self.assertLess(archive.stat().st_size, len(frames))
        self.invoke("restore", archive)
        self.assertEqual((run / "frames.pb").read_bytes(), frames)
        self.assertEqual((run / "summary.json").read_bytes(), summary)
        self.assertTrue((run / "empty").is_dir())
        self.invoke("restore", archive, success=False)
        self.invoke("pack", run, "--remove-source", success=False)
        self.assertTrue((run / "frames.pb").exists())

    def test_keeps_live_runs_and_baselines(self):
        run = self.evidence / "verify-live"
        run.mkdir()
        log = run / "run.log"
        log.write_bytes(b"build log")
        self.invoke("pack", log, "--remove-source", success=False)
        (run / "summary.json").write_text(json.dumps({"Status": "Running", "CompletedAt": None}))
        self.invoke("pack", run, "--remove-source", success=False)
        (run / "summary.json").write_text(
            json.dumps({"Status": "Pass", "CompletedAt": "2026-11-05T10:00:00Z"})
        )
        (run / "routine-inputs.json").write_bytes(b"baseline")
        self.invoke("pack", log, "--remove-source", success=False)
        self.invoke("pack", run, "--remove-source", success=False)
        self.assertTrue(log.exists())

    def test_keeps_canonical_campaign_inputs(self):
        coverage = self.root / "build" / "deck-coverage"
        coverage.mkdir()
        oracle_map = coverage / "oracle-map.tsv"
        oracle_map.write_bytes(b"active campaign input")
        self.invoke("pack", oracle_map, "--remove-source", success=False)
        self.assertTrue(oracle_map.exists())

    def test_pack_file_defaults_to_retaining_source(self):
        report = self.root / "build" / "campaign" / "old.report.json"
        report.parent.mkdir()
        report.write_bytes(b"closed report")
        self.invoke("pack", report)
        archive = report.with_name(report.name + ".zip")
        self.assertTrue(report.exists())
        report.unlink()
        self.invoke("restore", archive)
        self.assertEqual(report.read_bytes(), b"closed report")

    def test_rejects_escaping_and_corrupt_archives(self):
        archive = self.evidence / "closed.zip"
        manifest = ".cockatrice-evidence-manifest.json"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("closed/../../escape", b"bad")
            output.writestr(manifest, json.dumps({"closed/../../escape": "0" * 64}))
        self.invoke("restore", archive, success=False)
        self.assertFalse((self.evidence / "closed").exists())
        self.assertFalse((self.evidence.parent / "escape").exists())

        with zipfile.ZipFile(archive, "w") as output:
            output.writestr("closed", b"bad")
            output.writestr(manifest, json.dumps({"closed": "0" * 64}))
        result = self.invoke("restore", archive, success=False)
        self.assertIn("hash", result.stderr.lower())
        self.assertFalse((self.evidence / "closed").exists())


if __name__ == "__main__":
    unittest.main()
