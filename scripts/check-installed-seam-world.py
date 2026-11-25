#!/usr/bin/env python3
"""Opt-in QA: test installed seam saves without exposing test trust in product builds."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def varint(value):
    result = bytearray()
    while value >= 128:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def words(values):
    return b"".join(map(varint, values))


def field(number, payload):
    return varint(number * 8 + 2) + varint(len(payload)) + payload


def zigzag(value):
    return (value << 1) ^ (value >> 31)


def invented_tile(points):
    # Minimal synthetic MVT wire: one one-way LineString, no real positions.
    geom = [9, zigzag(points[0][0]), zigzag(points[0][1]), 8 * (len(points) - 1) + 2]
    for a, b in zip(points, points[1:]):
        geom.extend(zigzag(end - start) for start, end in zip(a, b))
    feature = words([8, 21, 24, 2]) + field(2, words([0, 0, 1, 1])) + field(4, words(geom))
    return field(
        3,
        words([120, 2, 40, 4096])
        + field(1, b"transportation")
        + field(2, feature)
        + field(3, b"class")
        + field(3, b"oneway")
        + field(4, field(1, b"secondary"))
        + field(4, words([40, 1])),
    )


def compact(value):
    return json.dumps(value, separators=(",", ":")).encode("ascii")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def synthetic_release():
    blobs = [
        invented_tile([[4176, 1800], [4156, 1790], [3856, 1640]]),
        invented_tile([[180, 1850], [80, 1800], [60, 1790], [-1385, 1067]]),
    ]
    revision = "rosenfu_synthetic_seam_qa_v1"
    tiles = [
        {
            "canonical_url": f"https://tiles.openfreemap.org/planet/{revision}/14/{x}/6451.pbf",
            "revision": revision,
            "z": 14,
            "x": x,
            "y": 6451,
            "raw_sha256": digest(raw),
            "decoder_rule": "ofm_transportation_z14_v1",
        }
        for x, raw in zip((14552, 14553), blobs)
    ]
    receipt = {
        "rule": "ofm_bus_westbound_shared_atom_z14_v1",
        "tilesWestEast": tiles,
        "rawFeatureId": 21,
        "verticesWestEast": [1, 2],
    }
    fixture = {
        "tiles": tiles,
        "blobs": [list(raw) for raw in blobs],
        "key": "ofm-westbound-v1:" + digest(compact(receipt)),
    }
    return compact({"version": 1, "receipts": [receipt]}), compact(fixture)


def cold_restart(command, scratch, env, test_name, isolated_network):
    # Find the compiled test executable from Cargo, then launch it directly:
    # no Cargo/package process, fixture blobs, or previous user profile at reopen.
    compilation = subprocess.run(
        command + ["--no-run", "--message-format=json"],
        cwd=scratch,
        env=env,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    artifacts = [json.loads(line) for line in compilation.stdout.splitlines()]
    [executable] = [
        item["executable"]
        for item in artifacts
        if item["reason"] == "compiler-artifact"
        and item["target"]["name"] == test_name
        and item.get("executable")
    ]
    cold_env = {
        key: env[key]
        for key in ("SystemRoot", "WINDIR", "PATH", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH")
        if key in env
    }
    profile = scratch / "cold-profile"
    for key in ("HOME", "USERPROFILE", "XDG_CACHE_HOME", "XDG_DATA_HOME",
                "XDG_CONFIG_HOME", "XDG_STATE_HOME", "LOCALAPPDATA", "APPDATA"):
        path = profile / key
        path.mkdir(parents=True)
        cold_env[key] = str(path)
    cold_env["ROSENFU_QA_SEAM_CHECKPOINT"] = env["ROSENFU_QA_SEAM_CHECKPOINT"]
    if isolated_network:
        cold_env["ROSENFU_QA_SEAM_REQUIRE_NO_ROUTE"] = "1"
    subprocess.run(
        (["unshare", "-Urn", "--"] if isolated_network else [])
        + [executable, "--exact", "cold_process_reopens_and_finishes_checkpoint", "--nocapture"],
        cwd=cold_env["HOME"],
        env=cold_env,
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--isolate-network", action="store_true",
        help="require a Linux user/network namespace for the cold reader (never silently falls back)",
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    # git archive admits only committed tracked files, never untracked user
    # data, hooks or credential settings. No clones/fetches across a network.
    with tempfile.TemporaryDirectory(prefix="rosenfu-installed-seam-qa-") as directory:
        scratch = Path(directory)
        with tempfile.TemporaryFile() as archive:
            subprocess.run(
                ["git", "archive", "--format=tar", "HEAD"],
                cwd=root,
                stdout=archive,
                check=True,
            )
            archive.seek(0)
            with tarfile.open(fileobj=archive) as tar:
                tar.extractall(scratch, filter="data")

        manifest = scratch / "crates/rosenfu-core/src/ofm_evidence/installed_westbound_seams.json"
        production_bytes = manifest.read_bytes()
        test_folder = scratch / "crates/rosenfu-storage/tests"
        test_folder.mkdir(exist_ok=True)
        test_name = "generated_installed_seam_world"
        shutil.copyfile(scratch / "scripts/qa/installed_seam_world.rs", test_folder / f"{test_name}.rs")
        cold_test = "generated_installed_seam_cold_reader"
        shutil.copyfile(scratch / "scripts/qa/installed_seam_cold_reader.rs", test_folder / f"{cold_test}.rs")
        synthetic_manifest, fixture = synthetic_release()
        (test_folder / f"{test_name}.json").write_bytes(fixture)

        # Never share a user/CI production Cargo target directory with the
        # patched-catalog binary. The explicit optional cache is QA-only.
        env = os.environ.copy()
        target = env.get("ROSENFU_SEAM_QA_TARGET_DIR")
        env["CARGO_TARGET_DIR"] = str(Path(target).resolve() if target else scratch / "qa-target")
        env["ROSENFU_QA_SEAM_CHECKPOINT"] = str(scratch / "checkpoint.json")
        env.setdefault("CARGO_BUILD_JOBS", "1")
        env.setdefault("CARGO_PROFILE_TEST_DEBUG", "0")
        env["CARGO_INCREMENTAL"] = "0"
        command = ["cargo", "test", "--locked", "--offline", "-p", "rosenfu-storage", "--test", test_name]

        manifest.write_bytes(synthetic_manifest)
        subprocess.run(
            command + ["installed_release_admission_full_file_roundtrip_and_overwrite_refusal"],
            cwd=scratch,
            env=env,
            check=True,
        )
        # The reader binary itself contains no embedded raw test fixture.
        cold_restart(command[:-1] + [cold_test], scratch, env, cold_test, args.isolate_network)
        manifest.write_bytes(production_bytes)
        subprocess.run(
            command + ["unmodified_release_rejects_the_checkpoint_without_overwrite"],
            cwd=scratch,
            env=env,
            check=True,
        )


if __name__ == "__main__":
    main()
