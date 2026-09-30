"""Offline readiness checks, immutable review bundles and measured command phases.

No source downloads, Git writes, card promotion or semantic approval. Use via the
PowerShell wrapper; final CardData validates the full evidence contract.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid

INTERACTIONS = ("timing", "simultaneous", "identity", "choices", "costs", "targets",
                "tokens", "presentation", "client")


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def sha(path):
    if not Path(path).exists():
        return None
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def located(parent, path):
    return (parent / path).resolve()


def cli(exe, *args):
    result = subprocess.run([str(exe), *map(str, args)], capture_output=True,
                            encoding="utf-8", errors="replace")
    if result.returncode:
        raise ValueError(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout)


def candidate_record(oracle_id, name, status, reason, dependencies):
    """Persist a human-assessed route, never infer readiness from a cache hit."""
    if not oracle_id.strip() or not name.strip() or not reason.strip():
        raise ValueError("candidate needs exact Oracle identity, name and reason")
    if status not in ("blocked", "held", "ready", "unassessed") or len(dependencies) < 2:
        raise ValueError("candidate needs a valid status, source and relevant contract dependencies")
    hashes = {}
    for path in dependencies:
        path = Path(path).resolve()
        if not path.is_file():
            raise ValueError(f"candidate dependency must be an existing file: {path}")
        hashes[str(path)] = sha(path)
    if len(hashes) < 2:
        raise ValueError("source and contract dependencies must be distinct")
    return {"version": 1, "oracle_id": oracle_id, "name": name, "status": status,
            "reason": reason, "dependencies": hashes, "semantic_approval": False}


def candidate_status(entry):
    if (entry.get("version") != 1 or not entry.get("oracle_id") or not entry.get("name") or
            entry.get("status") not in ("blocked", "held", "ready", "unassessed") or
            not entry.get("reason") or len(entry.get("dependencies", {})) < 2):
        raise ValueError("invalid candidate decision record")
    if any(not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest)
           for digest in entry["dependencies"].values()):
        raise ValueError("invalid candidate dependency digest")
    changed = [p for p, digest in entry["dependencies"].items() if sha(p) != digest]
    return {"oracle_id": entry["oracle_id"], "name": entry["name"], "reason": entry["reason"],
            "recorded_status": entry["status"], "effective_status": "unassessed" if changed else entry["status"],
            "changed_dependencies": changed, "semantic_approval": False,
            "meaning": "Selection aid only; recheck current registry, ownership, source and complete-card readiness before execution."}


def map_scaffold(typed, source, tests):
    """Use explicit presentation pointers, not Oracle text similarity, to draft line mappings."""
    if typed.get("name") != source.get("name") or not source.get("oracle_id"):
        raise ValueError("typed definition and exact Oracle source identity must match")
    faces = typed.get("faces", [])
    source_faces = source.get("card_faces") or [source]
    if not faces or len(faces) != len(source_faces):
        raise ValueError("typed/source face count mismatch")
    spans, catalogue, tokens = [], [], set()
    for index, (face, external) in enumerate(zip(faces, source_faces)):
        if face.get("name") != external.get("name") or not face.get("face_id"):
            raise ValueError("typed/source face identity mismatch")
        text = external.get("oracle_text", "").replace("\r\n", "\n").replace("\r", "\n")
        lines = [line.strip() for line in text.split("\n") if line.strip()]
        by_line = {n: [] for n in range(1, len(lines) + 1)}

        def walk(value, pointer):
            catalogue.append(pointer)
            if isinstance(value, dict):
                presentation = value.get("presentation")
                if isinstance(presentation, dict) and "OracleLines" in presentation:
                    numbers = presentation["OracleLines"]
                    if (not isinstance(numbers, list) or not numbers or
                            any(type(n) is not int or n not in by_line for n in numbers) or
                            numbers != sorted(set(numbers))):
                        raise ValueError(f"invalid OracleLines at {pointer}")
                    for number in numbers:
                        by_line[number].append(pointer)
                for key, child in value.items():
                    if key in ("CreateTokens", "CreateAttackingTokens") and isinstance(child, dict) and isinstance(child.get("token"), str):
                        tokens.add(child["token"])
                    walk(child, pointer + "/" + key.replace("~", "~0").replace("/", "~1"))
            elif isinstance(value, list):
                for n, child in enumerate(value):
                    walk(child, f"{pointer}/{n}")

        walk(face, f"/faces/{index}")
        for number, paths in by_line.items():
            span = dict(face_id=face["face_id"], start_line=number, end_line=number, typed_paths=paths)
            if not paths:
                span["unresolved_reason"] = "UNREVIEWED: map this source clause to the exact typed paths"
            spans.append(span)
    fixtures = []
    for test in tests:
        if not re.fullmatch(r"(?:scenario |[A-Za-z0-9_]+::)[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*", test):
            raise ValueError(f"noncanonical semantic test reference: {test}")
        fixtures.append({"test": test, "covers": "UNREVIEWED: specify independently asserted clause coverage"})
    return ({"format_version": 1, "oracle_id": source["oracle_id"], "spans": spans,
             "primitive_references": [], "tokens": sorted(tokens), "semantic_fixtures": fixtures,
             "complete_definition_review_confirmed": False}, catalogue)


def validate_assessments(packet, cards, rows, parent):
    identities = {card["id"]: card for card in packet["cards"]}
    seen = set()
    dependencies = []
    for card in cards:
        identity = card["id"]
        if identity not in identities or identity in seen:
            raise ValueError(f"unknown or duplicate assessed identity: {identity}")
        seen.add(identity)
        if card["route"] not in ("reuse_only", "new_composition") or card.get("unresolved"):
            raise ValueError(f"{identity}: runtime blockers/unresolved assessments cannot enter routine batches")
        if not isinstance(card.get("differences"), str) or not card["differences"].strip():
            raise ValueError(f"{identity}: explicit analogue differences required")
        checks = card.get("interactions", {})
        if set(checks) != set(INTERACTIONS) or any(not isinstance(v, str) or not v.strip() for v in checks.values()):
            raise ValueError(f"{identity}: every interaction needs a checked explanation or N/A reason")
        rulings = located(parent, card["rulings"])
        ruling_data = read(rulings)
        if not isinstance(ruling_data, dict) or not isinstance(ruling_data.get("data"), list):
            raise ValueError(f"{identity}: rulings must be a saved list response, including an empty list")
        dependencies.append(rulings)
        expected = {(line["face"], line["line"]) for line in identities[identity]["oracle_lines"]}
        covered = set()
        for clause in card["clauses"]:
            key = (clause["face"], clause["line"])
            if key not in expected or key in covered:
                raise ValueError(f"{identity}: duplicate or nonexistent source clause {key}")
            covered.add(key)
            row_indices = clause.get("row_indices", [])
            tests = clause.get("tests", [])
            if not row_indices and not tests:
                raise ValueError(f"{identity}: clause {key} has no row or dedicated test")
            for index in row_indices:
                if type(index) is not int or index < 0 or index >= len(rows) or rows[index]["card"] != identity:
                    raise ValueError(f"{identity}: row does not exercise this card")
            for test in tests:
                source = located(parent, test["source"])
                name = test["test"].split("::")[-1]
                if not test.get("target") or not test.get("package") or not re.search(r"\bfn\s+" + re.escape(name) + r"\s*\(", source.read_text(encoding="utf-8")):
                    raise ValueError(f"{identity}: dedicated test source/reference missing")
                dependencies.append(source)
        if covered != expected:
            raise ValueError(f"{identity}: all source faces/lines must have explicit evidence mappings")
    if not seen:
        raise ValueError("empty readiness assessment")
    return dependencies


def validate_maps(packet, cards, drafts, parent):
    by_id = {draft["id"]: draft for draft in drafts}
    sources = {source["id"]: source for source in packet["cards"]}
    paths = []
    for card in cards:
        draft = by_id[card["id"]]
        source = sources[card["id"]]
        if draft["name"] != source["name"]:
            raise ValueError("draft name must equal the exact source identity")
        path = located(parent, card["review_map"])
        mapping = read(path)
        if mapping.get("format_version") != 1 or mapping.get("oracle_id") != source["oracle_id"]:
            raise ValueError("review-map identity/version mismatch")
        covered = set()
        expected = {(line["face"], line["line"]) for line in source["oracle_lines"]}
        for span in mapping["spans"]:
            if span["face_id"] not in draft["faces"]:
                raise ValueError("review-map face identity mismatch")
            face = draft["faces"].index(span["face_id"])
            if span["start_line"] > span["end_line"] or not span["typed_paths"]:
                raise ValueError("empty map span or typed paths")
            for line in range(span["start_line"], span["end_line"] + 1):
                key = (face, line)
                if key not in expected or key in covered:
                    raise ValueError("overlapping or nonexistent map source line")
                covered.add(key)
            for pointer in span["typed_paths"]:
                if not isinstance(pointer, str) or not pointer.startswith("/"):
                    raise ValueError("typed path must be a JSON pointer")
                value = draft["typed"]
                try:
                    for part in pointer.split("/")[1:]:
                        part = part.replace("~1", "/").replace("~0", "~")
                        if isinstance(value, list):
                            if not re.fullmatch(r"0|[1-9][0-9]*", part):
                                raise ValueError("noncanonical array index")
                            value = value[int(part)]
                        else:
                            value = value[part]
                except (KeyError, IndexError, ValueError, TypeError) as error:
                    raise ValueError(f"unresolved typed map pointer: {pointer}") from error
        if covered != expected:
            raise ValueError("review-map must cover every source face/line")
        paths.append(path)
    return paths


def preflight(manifest, exe, output):
    manifest = manifest.resolve()
    parent = manifest.parent
    spec = read(manifest)
    if spec.get("version") != 1:
        raise ValueError("unsupported batch manifest version")
    packet_path = located(parent, spec["packet"])
    freshness_path = located(parent, spec["freshness"])
    freshness = cli(exe, "queue-check", "--entry", freshness_path)
    if not freshness["fresh"]:
        raise ValueError(f"stale preparation: {freshness['changed_dependencies']}")
    captured = read(freshness_path)["dependencies"]
    if not any(Path(path).is_file() and Path(path).samefile(packet_path) for path in captured):
        raise ValueError("preparation freshness must fingerprint the exact packet")
    draft_path = located(parent, spec["draft_batch"])
    validated = cli(exe, "validate-batch", "--batch", draft_path)
    draft = read(draft_path)
    packet = read(packet_path)
    dependencies = validate_assessments(packet, spec["cards"], draft["rows"], parent)
    selected = {card["id"] for card in spec["cards"]}
    if {card["id"] for card in validated["drafts"]} != selected:
        raise ValueError("assessed identities must equal the draft set")
    dependencies += validate_maps(packet, spec["cards"], validated["drafts"], parent)
    dependencies += [manifest, packet_path, freshness_path, draft_path]
    dependencies += [Path(card["path"]) for card in validated["drafts"]]
    dependencies += [located(parent, p) for p in spec.get("dependencies", [])]
    # Bind preparation dependencies too; a changed source/engine must invalidate the result.
    dependencies += [Path(p) for p in captured if Path(p).is_file()]
    result = {"version": 1, "semantic_approval": False, "structural_preflight": "pass",
              "cards": sorted(selected), "row_count": validated["row_count"],
              "dependencies": {str(p): sha(p) for p in dependencies},
              "preparation": str(freshness_path),
              "checker": str(exe.resolve()),
              "meaning": "Declared readiness and structural checks only; rows must execute and independent review/final gates remain required."}
    if any(value is None for value in result["dependencies"].values()):
        raise ValueError("missing batch dependency")
    if output.exists():
        raise ValueError("preflight output already exists; preserve earlier evidence")
    write(output, result)
    return result


def git(repo, *args, allow_diff=False):
    result = subprocess.run(["git", "--no-optional-locks", "-C", str(repo), *args], capture_output=True)
    if result.returncode not in ((0, 1) if allow_diff else (0,)):
        raise ValueError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout


def scoped(repo, name):
    path = repo / name
    # No reparse/symlink traversal for review snapshots.
    if any(p.is_symlink() for p in (path, *path.parents)):
        raise ValueError("review paths cannot traverse symlinks")
    path = path.resolve()
    if not path.is_relative_to(repo) or path == repo or ".git" in path.relative_to(repo).parts or path.is_dir():
        raise ValueError(f"review path outside file scope: {name}")
    return path


def freeze(repo, names, evidence, output):
    repo = repo.resolve()
    paths = [scoped(repo, name) for name in names]
    if not paths or len(set(paths)) != len(paths) or output.exists():
        raise ValueError("freeze requires unique explicit paths and a new output directory")
    # Complete all input checks before creating the bundle.
    logs = [Path(p).resolve() for p in evidence]
    for p in logs:
        if not p.is_file():
            raise ValueError(f"missing evidence {p}")
    dependencies = {str(p): sha(p) for p in paths + logs}
    base = git(repo, "rev-parse", "HEAD").decode().strip()
    relative = [p.relative_to(repo).as_posix() for p in paths]
    patch = git(repo, "diff", "--binary", "--no-ext-diff", "HEAD", "--", *relative)
    for path, rel in zip(paths, relative):
        if not git(repo, "ls-files", "--", rel).strip():
            if not path.exists():
                raise ValueError(f"untracked review path missing: {rel}")
            # Git recognizes /dev/null on Windows; return 1 is the expected diff result.
            patch += git(repo, "diff", "--no-index", "--binary", "--", "/dev/null", rel, allow_diff=True)
    output.mkdir()
    (output / "patch.diff").write_bytes(patch)
    for path, rel in zip(paths, relative):
        if path.exists():
            dest = output / "files" / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, dest)
    for i, path in enumerate(logs):
        dest = output / "evidence" / f"{i}-{path.name}"
        dest.parent.mkdir(exist_ok=True)
        shutil.copyfile(path, dest)
    if any(sha(path) != digest for path, digest in dependencies.items()):
        raise ValueError("scope changed while freezing; bundle is invalid")
    if git(repo, "rev-parse", "HEAD").decode().strip() != base:
        raise ValueError("HEAD changed while freezing; bundle is invalid")
    artifacts = {str(p.resolve()): sha(p) for p in output.rglob("*") if p.is_file()}
    result = {"version": 1, "base_sha": base, "paths": relative, "dependencies": dependencies,
              "artifacts": artifacts, "patch_sha256": sha(output / "patch.diff"), "semantic_approval": False}
    write(output / "manifest.json", result)
    return result


def fresh(manifest):
    changed = [path for path, digest in (manifest["dependencies"] | manifest.get("artifacts", {})).items()
               if sha(path) != digest]
    if manifest.get("preparation"):
        preparation = cli(manifest["checker"], "queue-check", "--entry", manifest["preparation"])
        changed += preparation["changed_dependencies"]
    return {"fresh": not changed, "changed_dependencies": changed, "semantic_approval": False}


def run_phase(repo, output, phase, command):
    if not command or not phase.strip():
        raise ValueError("phase and executable required")
    output.mkdir(parents=True, exist_ok=True)
    identifier = uuid.uuid4().hex
    log = output / f"{identifier}.log"
    started = datetime.now(timezone.utc)
    before = time.perf_counter()
    try:
        with log.open("wb") as stream:
            code = subprocess.run(command, cwd=repo, stdout=stream, stderr=subprocess.STDOUT).returncode
    except OSError as error:
        log.write_text(str(error), encoding="utf-8")
        code = 127
    result = {"phase": phase, "started_at": started.isoformat(), "completed_at": datetime.now(timezone.utc).isoformat(),
              "elapsed_seconds": time.perf_counter() - before, "exit_code": code, "command": command,
              "cwd": str(repo), "log": str(log.resolve()), "log_sha256": sha(log),
              "accounting": "Command wall time; not model effort, cost, or worker span."}
    write(output / f"{identifier}.json", result)
    if code:
        print(log.read_text(encoding="utf-8", errors="replace"), file=sys.stderr)
    return result


def doctor(repo, minimum_free_gib, inspect_git=True):
    if minimum_free_gib < 0:
        raise ValueError("disk threshold cannot be negative")
    free = shutil.disk_usage(repo).free / (1024 ** 3)
    result = {"free_gib": free, "minimum_free_gib": minimum_free_gib,
              "sufficient_disk": free >= minimum_free_gib}
    if inspect_git:
        path = located(repo, git(repo, "rev-parse", "--git-path", "index.lock").decode().strip())
        result["index_lock"] = {"path": str(path), "exists": path.exists(),
                                "bytes": path.stat().st_size if path.exists() else None,
                                "meaning": "Metadata only; presence/size does not prove staleness. Recovery requires quiescence and applicable authorization."}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    sub = parser.add_subparsers(dest="action", required=True)
    p = sub.add_parser("preflight")
    p.add_argument("--manifest", type=Path, required=True)
    p.add_argument("--exe", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("freeze")
    p.add_argument("--path", action="append", required=True)
    p.add_argument("--evidence", action="append", default=[])
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("check")
    p.add_argument("--manifest", type=Path, required=True)
    p = sub.add_parser("phase")
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--name", required=True)
    p.add_argument("command", nargs=argparse.REMAINDER)
    p = sub.add_parser("timing")
    p.add_argument("--directory", type=Path, required=True)
    p = sub.add_parser("doctor")
    p.add_argument("--minimum-free-gib", type=float, default=8)
    p = sub.add_parser("candidate-save")
    p.add_argument("--oracle-id", required=True)
    p.add_argument("--name", required=True)
    p.add_argument("--status", choices=("blocked", "held", "ready", "unassessed"), required=True)
    p.add_argument("--reason", required=True)
    p.add_argument("--depends", type=Path, action="append", required=True)
    p.add_argument("--out", type=Path, required=True)
    p = sub.add_parser("candidate-list")
    p.add_argument("--directory", type=Path, required=True)
    p = sub.add_parser("map-scaffold")
    p.add_argument("--typed", type=Path, required=True)
    p.add_argument("--source", type=Path, required=True)
    p.add_argument("--test", action="append", default=[])
    p.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "preflight":
        result = preflight(args.manifest, args.exe.resolve(), args.out)
    elif args.action == "freeze":
        result = freeze(args.repo, args.path, args.evidence, args.out)
    elif args.action == "check":
        manifest = read(args.manifest)
        result = fresh(manifest)
    elif args.action == "phase":
        command = args.command[1:] if args.command[:1] == ["--"] else args.command
        result = run_phase(args.repo, args.out, args.name, command)
    elif args.action == "doctor":
        result = doctor(args.repo, args.minimum_free_gib)
    elif args.action == "candidate-save":
        result = candidate_record(args.oracle_id, args.name, args.status, args.reason, args.depends)
        with args.out.open("x", encoding="utf-8") as destination:
            destination.write(json.dumps(result, indent=2) + "\n")
    elif args.action == "candidate-list":
        result = {"candidates": [candidate_status(read(p)) for p in sorted(args.directory.glob("*.json"))],
                  "semantic_approval": False}
    elif args.action == "map-scaffold":
        if args.out.resolve().is_relative_to((args.repo / "tricerules/tricerules-cards/data").resolve()):
            raise ValueError("scaffolds must stay outside embedded card data")
        mapping, catalogue = map_scaffold(read(args.typed), read(args.source), args.test)
        args.out.mkdir(parents=True, exist_ok=False)
        write(args.out / "review-map.json", mapping)
        write(args.out / "typed-paths.json", {"paths": catalogue, "typed_sha256": sha(args.typed),
                                            "source_sha256": sha(args.source), "semantic_approval": False})
        result = {"directory": str(args.out), "semantic_approval": False}
    else:
        phases = [read(p) for p in args.directory.glob("*.json")]
        result = {"phases": sorted(phases, key=lambda p: p["started_at"]),
                  "summed_command_seconds": sum(p["elapsed_seconds"] for p in phases),
                  "failed_commands": sum(p["exit_code"] != 0 for p in phases),
                  "accounting": "Includes failed attempts. Sum may include overlaps; it is not end-to-end authoring or active effort."}
    print(json.dumps(result, indent=2))
    if args.action == "phase":
        return result["exit_code"]
    return 0 if result.get("fresh", True) and result.get("sufficient_disk", True) else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError, IndexError) as error:
        print(f"Authoring batch failed: {error}", file=sys.stderr)
        sys.exit(1)
