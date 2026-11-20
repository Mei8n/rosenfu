"""Build and verify the Windows QA payload. Run after the installer smoke gate."""
import argparse
import hashlib
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import stat
import zipfile


def digest(stream):
    return hashlib.file_digest(stream, "sha256").hexdigest()


def path_digest(path):
    with path.open("rb") as stream:
        return digest(stream)


def inventory(stage):
    hashes = {}
    names = set()
    for item in sorted(stage.rglob("*")):
        if item.is_symlink() or item.is_junction():
            raise ValueError(f"Unexpected link in stage: {item}")
        name = item.relative_to(stage).as_posix()
        if (not item.is_file() and not item.is_dir()) or any(
            not re.fullmatch(r"[A-Za-z0-9@._+ -]+", part)
            or part[-1:] in (".", " ")
            or PureWindowsPath(part).is_reserved()
            for part in name.split("/")
        ):
            raise ValueError(f"Unsafe staged ZIP path: {name}")
        if name.split("/", 1)[0].casefold() == "package-sha256sums.txt":
            raise ValueError("Reserved ZIP manifest name already in stage")
        folded = name.casefold()
        if folded in names:
            raise ValueError(f"Case-colliding ZIP path: {name}")
        names.add(folded)
        if item.is_file():
            hashes[name] = path_digest(item)
    return hashes


def ci_build_context(environment):
    event = environment["GITHUB_EVENT_NAME"]
    if event not in ("push", "pull_request"):
        raise ValueError("Unsupported QA build event")
    return {
        "commit": environment["GITHUB_SHA"],
        "kind": "integration" if event == "push" else "preview",
        "ref": environment["GITHUB_REF"],
        "run": environment["GITHUB_RUN_ID"],
        "attempt": environment["GITHUB_RUN_ATTEMPT"],
    }


def zip_entry(name):
    entry = zipfile.ZipInfo(name, (2020, 1, 1, 0, 0, 0))
    entry.compress_type = zipfile.ZIP_DEFLATED
    entry.external_attr = (stat.S_IFREG | 0o644) << 16
    return entry


def build(stage, output, expected):
    if stage.is_symlink() or stage.is_junction() or not stage.is_dir() or output.is_symlink() or output.exists():
        raise ValueError("Expected an ordinary existing staging directory and new ZIP path")
    stage = stage.resolve()
    output = output.resolve()
    if output.is_relative_to(stage):
        raise ValueError("ZIP must be outside staging directory")
    files = inventory(stage)
    mandatory = {
        "BUILD.txt", "SHA256SUMS.txt", "windows-alpha02-test-play.txt",
        "data-and-notices/road-data/manifest.json",
        "data-and-notices/water-japan-coarse-20261114/manifest.json",
        "data-and-notices/LINE-Seed-JP-OFL.txt",
        "data-and-notices/software/DEPENDENCIES.json",
        "data-and-notices/software/README.txt",
    }
    installer = [name for name in files if re.fullmatch(
        r"Rosenfu-alpha02-(?:preview|integration)-[a-f0-9]{12}-windows-x64-setup\.exe", name
    )]
    if mandatory - files.keys() or len(installer) != 1:
        raise ValueError("Incomplete stage or ambiguous installer")
    name = installer[0]
    metadata = {}
    for line in (stage / "BUILD.txt").read_text(encoding="utf-8-sig").splitlines():
        key, found, value = line.partition("=")
        if found:
            if key in metadata:
                raise ValueError(f"Duplicate build metadata field: {key}")
            metadata[key] = value
    commit = metadata.get("commit", "")
    kind = metadata.get("kind", "")
    if not re.fullmatch(r"[a-f0-9]{40}", commit) or kind not in {"preview", "integration"}:
        raise ValueError("Missing exact SHA/kind build metadata")
    if name != f"Rosenfu-alpha02-{kind}-{commit[:12]}-windows-x64-setup.exe":
        raise ValueError("Installer name is inconsistent with the full source SHA")
    ref = metadata.get("ref", "")
    if kind == "integration" and ref != "refs/heads/dev/alpha-0.2-integration":
        raise ValueError("Integration payload does not come from the alpha 0.2 branch")
    if kind == "preview" and not re.fullmatch(r"refs/pull/[1-9][0-9]*/merge", ref):
        raise ValueError("Preview payload does not come from a PR merge ref")
    if any(not re.fullmatch(r"[1-9][0-9]{0,19}", metadata.get(key, "")) for key in ("run", "attempt")):
        raise ValueError("Invalid GitHub run/attempt metadata")
    if metadata.get("audience") != "internal-QA-only":
        raise ValueError("Invalid QA audience")
    if any(metadata[key] != expected[key] for key in ("commit", "kind", "ref", "run", "attempt")):
        raise ValueError("Build metadata does not match the GitHub workflow context")
    if any(key.lower().endswith(".exe") for key in files.keys() - {name}):
        raise ValueError("Unexpected executable in QA payload")
    if (stage / "SHA256SUMS.txt").read_text(encoding="ascii").strip() != f"{files[name]}  {name}":
        raise ValueError("Installer checksum mismatch")
    manifest = "".join(f"{value}  {key}\n" for key, value in sorted(files.items()))
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for key in sorted(files):
            # Don't archive filesystem modes or symlinks; fixed timestamp gives stable metadata.
            entry = zip_entry(key)
            with (stage / key).open("rb") as source, archive.open(entry, "w") as destination:
                shutil.copyfileobj(source, destination)
        archive.writestr(zip_entry("PACKAGE-SHA256SUMS.txt"), manifest)
    with zipfile.ZipFile(output) as archive:
        descriptors = archive.infolist()
        if len(descriptors) != len(files) + 1 or set(archive.namelist()) != set(files) | {"PACKAGE-SHA256SUMS.txt"}:
            raise ValueError("ZIP member inventory mismatch")
        if len({member.filename.casefold() for member in descriptors}) != len(descriptors):
            raise ValueError("Case-colliding ZIP entries")
        if any(stat.S_ISLNK(member.external_attr >> 16) or member.is_dir() for member in descriptors):
            raise ValueError("Unexpected ZIP link or directory record")
        if archive.read("PACKAGE-SHA256SUMS.txt").decode("ascii") != manifest:
            raise ValueError("ZIP manifest mismatch")
        for key, expected in files.items():
            with archive.open(key) as member:
                if digest(member) != expected:
                    raise ValueError(f"ZIP hash mismatch: {key}")
    print(f"{path_digest(output)}  {output.name}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("stage", type=Path)
    parser.add_argument("output", type=Path)
    options = parser.parse_args()
    build(options.stage, options.output, ci_build_context(os.environ))
