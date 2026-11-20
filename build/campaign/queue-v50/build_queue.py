#!/usr/bin/env python3
"""Build the queue-v50 source/rulings packets from the pinned datasets (read-only join)."""
from __future__ import annotations

import gzip
import hashlib
import json
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
BUILD = ROOT / "build" / "campaign"
OUT = BUILD / "queue-v50"
BULK = ROOT / "oracle-cards.jsonl.gz"
RULINGS = BUILD / "source" / "rulings.jsonl.gz"
DATA = ROOT / "tricerules" / "tricerules-cards" / "data"
VALIDATION = BUILD / "source" / "cards-validation.xml"
CARDS_MD = ROOT / "tricerules" / "CARDS.md"

BULK_SHA = "a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853"
RULINGS_SHA = "3ee07dbf91347f7940b4067ab1c01e0d5097da8f6c27ef620c202ed4262aaf1f"

FAMILIES = {
    "01-put-on-top": [
        "Repel", "Grasp of Phantoms", "Run Aground",
        "Banishment Decree", "Temporal Eddy", "Fallow Earth",
    ],
    "02-graveyard-to-hand": [
        "Boggart Birth Rite", "Argivian Find",
        "Nature's Spiral", "Déjà Vu",
    ],
    "03-tap-stun-spells": [
        "Cryogenic Stasis", "Impede Momentum", "Tranquilize", "Freeze in Place",
    ],
    "04-etb-shrink": [
        "Eyeblight Assassin", "Black Dragon", "Faerie Duelist", "Brinebarrow Intruder",
    ],
    "05-etb-pump-target": [
        "Haazda Officer", "Herald of the Fair", "Rage-Scarred Berserker",
    ],
    "06-etb-counter-another": [
        "Stromkirk Mentor", "Sanguine Glorifier", "Jade Bearer",
    ],
    "07-basic-land-to-hand": [
        "Gatecreeper Vine", "District Guide", "Farfinder", "Pilgrim's Eye",
    ],
    "08-treasure-on-death": [
        "Dire Fleet Hoarder", "Jewel-Eyed Cobra", "Patron of the Arts",
    ],
    "09-destroy-artifact-enchant": [
        "Naturalize", "Nature's Chant", "Return to the Earth",
    ],
}


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def slugify(name: str) -> str:
    folded = unicodedata.normalize("NFD", name.lower())
    text = "".join(ch for ch in folded if not unicodedata.combining(ch))
    return "_".join(text.replace("'", "").replace("/", " ").split())


def norm_text(card: dict) -> str:
    parts = [card.get("oracle_text", "")]
    for face in card.get("card_faces") or []:
        parts.append(face.get("oracle_text", ""))
    return re.sub(r"\s+", " ", "\n".join(parts)).strip().lower()


def main() -> int:
    if sha256(BULK) != BULK_SHA:
        raise SystemExit("bulk SHA mismatch")
    if sha256(RULINGS) != RULINGS_SHA:
        raise SystemExit("rulings SHA mismatch")

    by_name: dict[str, list[dict]] = defaultdict(list)
    with gzip.open(BULK, "rt", encoding="utf-8") as fh:
        for line in fh:
            card = json.loads(line)
            by_name[card["name"]].append(card)
    rulings: dict[str, list[dict]] = defaultdict(list)
    with gzip.open(RULINGS, "rt", encoding="utf-8") as fh:
        for line in fh:
            row = json.loads(line)
            rulings[row["oracle_id"]].append(row)

    registry_names = set()
    registry_slugs = set()
    for path in sorted(DATA.rglob("*.ron")):
        if "tokens" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(r'\bname:\s*"((?:[^"\\]|\\.)*)"', text):
            registry_names.add(match.group(1).replace('\\"', '"').lower())
        registry_slugs.add(path.stem)

    blocked = set()
    for line in (BUILD / "blocked.tsv").read_text(encoding="utf-8").splitlines()[1:]:
        fields = line.split("\t")
        if fields and fields[0]:
            blocked.add(fields[0])

    exclusions = set()
    for line in (BUILD / "queue-exclusions.txt").read_text(encoding="utf-8").splitlines():
        value = line.strip()
        if value and not value.startswith("#"):
            exclusions.add(value.lower())

    done = set()
    undone = set()
    for line in CARDS_MD.read_text(encoding="utf-8").splitlines():
        if line.startswith("- [x] "):
            done.add(line[6:].strip().lower())
        elif line.startswith("- [ ] "):
            undone.add(line[6:].strip().lower())

    validation = VALIDATION.read_text(encoding="utf-8", errors="replace")

    ambiguous = set()
    for name, group in by_name.items():
        if len({norm_text(card) for card in group}) > 1:
            ambiguous.add(name)

    revalidation_rows = []
    cards_rows = []
    seen_ids = set()
    seen_names = set()
    for family, names in FAMILIES.items():
        family_dir = OUT / "families" / family
        (family_dir / "source").mkdir(parents=True, exist_ok=True)
        (family_dir / "rulings").mkdir(parents=True, exist_ok=True)
        for name in names:
            group = by_name.get(name, [])
            ok = [
                c
                for c in group
                if c.get("legalities", {}).get("commander") == "legal"
                and c.get("layout") == "normal"
                and not c.get("card_faces")
            ]
            if len(ok) != 1:
                raise SystemExit(f"{name}: expected one normal commander-legal record, found {len(ok)}")
            card = ok[0]
            oid = card["oracle_id"]
            failures = []
            if name.lower() in registry_names:
                failures.append("already in registry")
            if name.lower() in exclusions:
                failures.append("in queue-exclusions")
            if oid in blocked:
                failures.append("in blocked.tsv")
            if name in ambiguous:
                failures.append("ambiguous presentation name")
            if name.lower() not in undone:
                failures.append("not an unchecked CARDS.md row")
            if f"<name>{name}</name>" not in validation:
                failures.append("not in cards-validation.xml")
            if oid in seen_ids or name.lower() in seen_names:
                failures.append("duplicate candidate")
            if slugify(name) in registry_slugs:
                failures.append("slug already in registry")
            if failures:
                raise SystemExit(f"{name}: {'; '.join(failures)}")
            seen_ids.add(oid)
            seen_names.add(name.lower())
            (family_dir / "source" / f"{oid}.json").write_text(
                json.dumps(card, indent=1, ensure_ascii=False), encoding="utf-8"
            )
            rows = rulings.get(oid, [])
            (family_dir / "rulings" / f"{oid}.jsonl").write_text(
                "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows),
                encoding="utf-8",
            )
            oracle_flat = re.sub(r"\s+", " ", card.get("oracle_text", "")).strip()
            revalidation_rows.append([
                family, oid, name, card.get("mana_cost", ""), card.get("type_line", ""),
                card.get("power", ""), card.get("toughness", ""), card.get("layout", ""),
                card.get("legalities", {}).get("commander", ""), oracle_flat,
                str(len(rows)),
            ])
            cards_rows.append([family, oid, name])

    header = [
        "family", "oracle_id", "name", "mana_cost", "type_line", "power", "toughness",
        "layout", "commander", "oracle_text", "rulings",
    ]
    (OUT / "revalidation.tsv").write_text(
        "\t".join(header) + "\n" + "\n".join("\t".join(r) for r in revalidation_rows) + "\n",
        encoding="utf-8",
    )
    (OUT / "cards.tsv").write_text(
        "family\toracle_id\tname\n"
        + "\n".join("\t".join(r) for r in cards_rows)
        + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(cards_rows)} candidates in {len(FAMILIES)} families")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
