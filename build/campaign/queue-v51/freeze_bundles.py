#!/usr/bin/env python3
"""Freeze hash-verified queue-v51 review bundles."""
from __future__ import annotations

import hashlib
import json
import subprocess
import unicodedata
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
OUT = ROOT / "build" / "campaign" / "queue-v51" / "freeze"
OUT.mkdir(parents=True, exist_ok=True)

BATCHES = {
    "batch-a": ["01-token-spells", "02-splicer-cycle", "03-artifact-value-tutors"],
    "batch-b": ["04-subtype-harbingers", "05-aura-restrictions", "06-etb-each-discard"],
    "batch-c": ["08-attack-team-pumps", "09-spirit-arcane-payoffs"],
}

SHARED = [
    "tricerules/tricerules-core/tests/scenario/q51_cohorts_batch.rs",
    "tricerules/tricerules-core/tests/scenario.rs",
    "tricerules/tricerules-cards/tests/q51_cohorts_batch.rs",
    "tricerules/tricerules-cards/tests/suites/mechanic_registry.rs",
    "tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv",
    "tricerules/tricerules-cards/authoring/presentation-exceptions.tsv",
    "tricerules/CARDS.md",
    "build/campaign/blocked.tsv",
    "build/campaign/queue-v51/build_queue.py",
    "build/campaign/queue-v51/gen_ron.py",
    "build/campaign/queue-v51/gen_maps.py",
    "build/campaign/queue-v51/gen_family_records.py",
    "build/campaign/queue-v51/freeze_bundles.py",
    "build/campaign/queue-v51/cards.tsv",
    "build/campaign/queue-v51/revalidation.tsv",
    "build/campaign/queue-v51/selection.md",
    "build/campaign/queue-v51/report.md",
    "build/campaign/queue-v51/map-list.json",
]

TOKENS = {
    "01-token-spells": [
        "centaur_g_3_3", "elemental_ur_4_4", "wurm_g_5_5_trample", "servo_c_1_1",
        "soldier_w_1_1_lifelink", "citizen_w_1_1", "minotaur_r_2_3_haste",
        "warrior_w_1_1", "saproling_g_1_1", "myr_token_c_1_1", "knight_w_2_2_vigilance",
    ],
    "02-splicer-cycle": ["golem_c_3_3"],
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def slug(name: str) -> str:
    folded = unicodedata.normalize("NFD", name.lower())
    text = "".join(ch for ch in folded if not unicodedata.combining(ch))
    return "_".join(text.replace("'", "").replace("/", " ").split())


def card_ids(family: str) -> list[str]:
    return [
        slug(name)
        for fam, oid, name in (
            line.split("\t")
            for line in (ROOT / "build/campaign/queue-v51/cards.tsv").read_text().splitlines()[1:]
        )
        if fam == family
    ]


def tracked_files(family: str) -> list[str]:
    paths = []
    for cid in card_ids(family):
        paths.append(f"tricerules/tricerules-cards/data/{cid}.ron")
        paths.append(f"tricerules/tricerules-cards/authoring/review-maps/{cid}.json")
    for token in TOKENS.get(family, []):
        paths.append(f"tricerules/tricerules-cards/data/tokens/{token}.ron")
    paths.append(f"build/campaign/queue-v51/families/{family}/family.md")
    family_dir = ROOT / f"build/campaign/queue-v51/families/{family}"
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
            "scenario_green": "build/verification-logs/20261120-q51-scenario-green4.log",
            "registry_green": "build/verification-logs/20261120-q51-registry-green4.log",
            "intended_red": ["build/verification-logs/20261120-q51-intended-red.log"],
            "evidence_preparation": "build/verification-logs/card-evidence-f69e990e-c0e2-4ee8-8a8d-da4cc83beba4",
            "metadata_refresh": "build/verification-logs/card-data-e756693d-c2c1-47a2-83ba-6aedef321489",
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
