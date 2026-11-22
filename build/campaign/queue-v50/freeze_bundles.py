#!/usr/bin/env python3
"""Freeze hash-verified queue-v50 review bundles."""
from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
OUT = ROOT / "build" / "campaign" / "queue-v50" / "freeze"
OUT.mkdir(parents=True, exist_ok=True)

BATCHES = {
    "batch-a": [
        "01-put-on-top", "02-graveyard-to-hand", "03-tap-stun-spells",
    ],
    "batch-b": [
        "04-etb-shrink", "05-etb-pump-target", "06-etb-counter-another",
    ],
    "batch-c": [
        "07-basic-land-to-hand", "08-treasure-on-death", "09-destroy-artifact-enchant",
    ],
}

SHARED = [
    "tricerules/tricerules-core/tests/scenario/q50_cohorts_batch.rs",
    "tricerules/tricerules-core/tests/scenario.rs",
    "tricerules/tricerules-cards/tests/q50_cohorts_batch.rs",
    "tricerules/tricerules-cards/tests/suites/mechanic_registry.rs",
    "tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv",
    "tricerules/tricerules-cards/authoring/presentation-exceptions.tsv",
    "tricerules/tricerules-core/tests/conformance/baseline.tsv",
    "tricerules/CARDS.md",
    "build/campaign/blocked.tsv",
    "build/campaign/queue-v50/build_queue.py",
    "build/campaign/queue-v50/gen_ron_batch_a.py",
    "build/campaign/queue-v50/gen_ron_batch_b.py",
    "build/campaign/queue-v50/gen_ron_batch_c.py",
    "build/campaign/queue-v50/gen_maps.py",
    "build/campaign/queue-v50/gen_family_records.py",
    "build/campaign/queue-v50/cards.tsv",
    "build/campaign/queue-v50/revalidation.tsv",
    "build/campaign/queue-v50/selection.md",
    "build/campaign/queue-v50/report.md",
    "build/campaign/next/HANDOFF-queue-v50.md",
]


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def card_ids(family: str) -> list[str]:
    rows = []
    for line in (ROOT / "build/campaign/queue-v50/cards.tsv").read_text().splitlines()[1:]:
        fam, oid, name = line.split("\t")
        if fam != family:
            continue
        cid = name.lower().replace("'", "").replace("/", " ").replace(" ", "_").replace(",", "")
        for source, target in (("é", "e"), ("è", "e"), ("ê", "e"), ("à", "a"), ("â", "a"), ("ô", "o")):
            cid = cid.replace(source, target)
        rows.append(cid)
    return rows


def tracked_files(family: str) -> list[str]:
    paths = []
    for cid in card_ids(family):
        paths.append(f"tricerules/tricerules-cards/data/{cid}.ron")
        paths.append(f"tricerules/tricerules-cards/authoring/review-maps/{cid}.json")
    paths.append(f"build/campaign/queue-v50/families/{family}/family.md")
    family_dir = ROOT / f"build/campaign/queue-v50/families/{family}"
    for path in sorted((family_dir / "source").glob("*.json")):
        paths.append(str(path.relative_to(ROOT)))
    for path in sorted((family_dir / "rulings").glob("*.jsonl")):
        paths.append(str(path.relative_to(ROOT)))
    return paths


def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True)


base = git("rev-parse", "HEAD").strip()
for batch, families in BATCHES.items():
    paths = SHARED.copy()
    for family in families:
        paths += tracked_files(family)
    paths = sorted(set(paths))
    manifest = {
        "format_version": 1,
        "bundle": batch,
        "families": families,
        "base_head": base,
        "review_type": "frozen implementation review",
        "test_evidence": {
            "scenario_green": "build/verification-logs/20261120-163633-114-q50-scenario-green-v4.log",
            "registry_green": "build/verification-logs/20261120-163633-113-q50-registry-green-v4.log",
            "intended_red": [f"build/verification-logs/{name}" for name in sorted(p.name for p in (ROOT / "build/verification-logs").glob("*q50-intended-red*.log"))],
            "evidence_preparation": "build/verification-logs/card-evidence-6e6ed57a-3ed0-4a08-9a24-329fdc3d6dd3",
            "metadata_refresh": "build/verification-logs/card-data-1cede215-79bc-4497-8218-87a48af05507",
        },
        "files": [],
        "dependencies": {},
        "artifacts": {},
    }
    patch_parts = []
    for rel in paths:
        path = ROOT / rel
        if not path.is_file():
            raise SystemExit(f"missing frozen file: {rel}")
        digest = sha256(path)
        manifest["files"].append({"path": rel, "sha256": digest, "bytes": path.stat().st_size})
        manifest["dependencies"][rel] = digest
        if rel in git("ls-files", "--", rel).splitlines():
            patch_parts.append(git("diff", "--", rel))
        else:
            proc = subprocess.run(
                ["git", "diff", "--no-index", "--", "/dev/null", rel],
                cwd=ROOT, text=True, capture_output=True,
            )
            patch_parts.append(proc.stdout)
    patch_path = OUT / f"{batch}.patch.diff"
    patch_path.write_text("".join(patch_parts), encoding="utf-8")
    manifest["artifacts"][patch_path.name] = sha256(patch_path)
    (OUT / f"{batch}.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"{batch}: {len(paths)} files, patch {sha256(patch_path)[:12]}")
