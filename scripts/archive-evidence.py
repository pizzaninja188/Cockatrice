"""Pack and restore one closed, local Cockatrice evidence file or directory."""

import argparse
import hashlib
import json
import os
import shutil
import stat
import sys
import uuid
import zipfile
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
EVIDENCE_ROOTS = ("campaign", "deck-coverage", "verification-logs")
MANIFEST = ".cockatrice-evidence-manifest.json"
PROTECTED = {"routine-inputs.json", "oracle-map.tsv"}
WINDOWS_DEVICES = {"CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"} | {
    f"{prefix}{number}" for prefix in ("COM", "LPT") for number in "123456789¹²³"
}


def evidence_stat(path):
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT:
        raise ValueError(f"Linked evidence path: {path}")
    if not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
        raise ValueError(f"Unsupported evidence path: {path}")
    return info


def locate(argument):
    path = Path(os.path.abspath(REPOSITORY / argument))
    parts = path.relative_to(REPOSITORY).parts
    if len(parts) < 3 or parts[0] != "build" or parts[1] not in EVIDENCE_ROOTS:
        raise ValueError("Select one item inside build/campaign, build/deck-coverage or build/verification-logs.")
    current = REPOSITORY
    for part in parts:
        current /= part
        evidence_stat(current)
    return path, REPOSITORY.joinpath(*parts[:2])


def member_path(name, root, directory):
    parts = name.removesuffix("/").split("/") if directory else name.split("/")
    if parts[0] != root or any(
        part in ("", ".", "..")
        or part.rstrip(" .") != part
        or part.split(".")[0].upper() in WINDOWS_DEVICES
        or any(character in '<>:"\\|?*' or ord(character) < 32 for character in part)
        for part in parts
    ):
        raise ValueError(f"Invalid archive member: {name!r}")
    return Path(*parts)


def inventory(source):
    pending = [source]
    entries = {}
    while pending:
        path = pending.pop()
        info = evidence_stat(path)
        if path.name.lower() in PROTECTED:
            raise ValueError(f"Protected baseline or campaign input: {path}")
        name = path.relative_to(source.parent).as_posix()
        if stat.S_ISDIR(info.st_mode):
            if path.is_mount():
                raise ValueError(f"Mounted evidence directory: {path}")
            name += "/"
            pending.extend(sorted(path.iterdir(), reverse=True))
        member_path(name, source.name, name.endswith("/"))
        entries[name] = (info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
    return entries


def require_closed(source, root):
    for path in (source, *source.parents):
        if path == root:
            break
        if (path / "routine-inputs.json").exists():
            raise ValueError(f"Protected verification baseline: {path}")
        if root.name == "verification-logs" and path.parent == root and path.name.startswith("verify-"):
            summary_path = path / "summary.json"
            evidence_stat(summary_path)
            summary = json.loads(summary_path.read_bytes())
            if (
                not isinstance(summary, dict)
                or summary.get("Status") not in ("Pass", "Fail")
                or not summary.get("CompletedAt")
            ):
                raise ValueError(f"Verification run has not completed: {path}")


def hash_stream(source, destination=None):
    digest = hashlib.sha256()
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(chunk)
        if destination is not None:
            destination.write(chunk)
    return digest.hexdigest()


def read_archive(path, root_name, destination=None):
    with zipfile.ZipFile(path) as archive:
        members = archive.infolist()
        names = {entry.filename for entry in members}
        if len(names) != len(members) or MANIFEST not in names:
            raise ValueError("Missing archive manifest or duplicate members.")
        manifest = json.loads(archive.read(MANIFEST))
        if not isinstance(manifest, dict) or not manifest or set(manifest) != names - {MANIFEST}:
            raise ValueError("Archive contents differ from the manifest.")
        if (root_name in names) == (root_name + "/" in names):
            raise ValueError("Archive must contain exactly one root file or directory.")
        destinations = set()
        for entry in sorted(members, key=lambda item: item.filename):
            name = entry.filename
            if name == MANIFEST:
                continue
            relative = member_path(entry.orig_filename, root_name, entry.is_dir())
            if len(relative.parts) > 1:
                if relative.parent.as_posix() + "/" not in names:
                    raise ValueError(f"Missing parent directory for archive member: {name}")
            folded = relative.as_posix().casefold()
            if folded in destinations:
                raise ValueError(f"Duplicate archive destination: {relative}")
            destinations.add(folded)
            if entry.is_dir():
                if manifest[name] is not None or entry.file_size:
                    raise ValueError(f"Invalid directory entry: {name}")
                if destination is not None:
                    (destination / relative).mkdir(parents=True, exist_ok=True)
                continue
            with archive.open(entry) as original:
                if destination is None:
                    actual = hash_stream(original)
                else:
                    target = destination / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with target.open("xb") as output:
                        actual = hash_stream(original, output)
            if actual != manifest[name]:
                raise ValueError(f"SHA-256 hash mismatch: {name}")


def pack(source, evidence_root, remove_source):
    require_closed(source, evidence_root)
    if source.name == MANIFEST or source.suffix.lower() == ".zip":
        raise ValueError("Select an uncompressed evidence item.")
    archive = source.with_name(source.name + ".zip")
    if os.path.lexists(archive):
        raise FileExistsError(f"Archive already exists: {archive}")
    entries = inventory(source)
    staging = archive.with_name(f".evidence-{uuid.uuid4()}.tmp")
    temporary = staging.open("xb")
    try:
        with temporary:
            manifest = {}
            with zipfile.ZipFile(temporary, "w") as output:
                for name in entries:
                    path = source.parent / member_path(name, source.name, name.endswith("/"))
                    if name.endswith("/"):
                        output.writestr(name, b"")
                        manifest[name] = None
                        continue
                    info = zipfile.ZipInfo.from_file(path, name, strict_timestamps=False)
                    info.compress_type = zipfile.ZIP_DEFLATED
                    with path.open("rb") as original, output.open(info, "w", force_zip64=True) as entry:
                        manifest[name] = hash_stream(original, entry)
                output.writestr(MANIFEST, json.dumps(manifest), compress_type=zipfile.ZIP_DEFLATED)
            temporary.flush()
            os.fsync(temporary.fileno())
        read_archive(staging, source.name)
        if inventory(source) != entries:
            raise ValueError("Evidence changed during archiving.")
        # Publish the verified archive without overwriting a concurrent publication.
        if os.name == "nt":
            staging.rename(archive)
        else:
            os.link(staging, archive)
    finally:
        staging.unlink(missing_ok=True)
    print(f"Verified archive {archive}")
    if remove_source:
        if source.is_dir():
            shutil.rmtree(source)
        else:
            source.unlink()
        print(f"Removed archived source {source}")


def restore(archive):
    if archive.suffix.lower() != ".zip":
        raise ValueError("Select a Cockatrice evidence ZIP.")
    destination = archive.with_suffix("")
    if os.path.lexists(destination):
        raise FileExistsError(f"Restore destination already exists: {destination}")
    staging = destination.with_name(f".evidence-restore-{uuid.uuid4()}")
    staging.mkdir()
    try:
        read_archive(archive, destination.name, staging)
        restored = staging / destination.name
        if os.path.lexists(destination):
            raise FileExistsError(f"Restore destination already exists: {destination}")
        if os.name != "nt" and restored.is_file():
            os.link(restored, destination)
        else:
            restored.rename(destination)
    finally:
        shutil.rmtree(staging)
    print(f"Verified and restored {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pack_command = commands.add_parser("pack", help="Compress a closed evidence item into a verified ZIP.")
    pack_command.add_argument("path", type=Path)
    pack_command.add_argument("--remove-source", action="store_true", help="Reclaim the source after the archive is verified.")
    restore_command = commands.add_parser("restore", help="Verify a ZIP and restore the original evidence path.")
    restore_command.add_argument("path", type=Path)
    options = parser.parse_args()
    try:
        path, evidence_root = locate(options.path)
        if options.command == "pack":
            pack(path, evidence_root, options.remove_source)
        else:
            restore(path)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
