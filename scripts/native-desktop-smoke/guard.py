"""Fail before building unless this is the reviewed, same-repo one-off target."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

BASE="83fe184bc962861d11d3b65ab955646da1c65a4a"
LABEL="qa:native-desktop-83fe184bc962"
REPO="Mei8n/rosenfu"
HEAD_REF="work/alpha02-road-reuse"
BASE_REF="dev/alpha-0.2-integration"
PATHS={
    ".github/workflows/native-desktop-smoke.yml",
    *("scripts/native-desktop-smoke/"+p for p in (
        "README.md","fixture.py","guard.py","run.sh","smoke.py","smoke_tram.py","smoke_authored_tram.py",
        "test_guard.py","test_smoke.py","wd_native.py","seed/Cargo.toml",
        "seed/Cargo.lock","seed/src/main.rs"
    )),
}


def check_event(event, environment):
    if environment.get("GITHUB_EVENT_NAME")!="pull_request" or environment.get("GITHUB_RUN_ATTEMPT")!="1":
        raise ValueError("not the first direct pull_request run")
    if environment.get("GITHUB_REPOSITORY")!=REPO:
        raise ValueError("not the reviewed repository")
    pr=event["pull_request"]
    if (event["action"]!="labeled" or event["label"]["name"]!=LABEL or
        event["number"]!=285 or pr["number"]!=285 or pr["draft"] is not True or pr["state"]!="open"):
        raise ValueError("not the explicit label on the open draft")
    head,base=pr["head"],pr["base"]
    if (head["ref"]!=HEAD_REF or base["ref"]!=BASE_REF or
        head["repo"]["full_name"]!=REPO or base["repo"]["full_name"]!=REPO):
        raise ValueError("unreviewed branch or fork")
    sha=head["sha"]
    if not isinstance(sha,str) or re.fullmatch(r"[0-9a-f]{40}",sha) is None:
        raise ValueError("invalid SHA")
    return sha


def check_checkout(head, parents, changes):
    if parents!=[head,BASE] or set(changes)!=PATHS or len(changes)!=len(PATHS):
        raise ValueError("expected one exact smoke-only child of reviewed product SHA")


def main():
    def git(*args):
        # Static argv, never a subprocess shell or evaluation of event prose.
        return subprocess.check_output(["git",*args])
    if os.environ.get("GITHUB_ACTIONS")!="true":
        raise ValueError("this job is deliberately CI-only")
    raw=Path(os.environ["GITHUB_EVENT_PATH"]).read_bytes()
    if len(raw)>2_000_000:
        raise ValueError("oversized pull request event")
    head=check_event(json.loads(raw),os.environ)
    if git("rev-parse","HEAD").decode().strip()!=head:
        raise ValueError("not the exact label event checkout")
    if git("status","--porcelain","--untracked-files=no"):
        raise ValueError("tracked worktree differs from immutable candidate")
    # Check raw NUL-separated names, independent of Git quoting configuration.
    parents=git("rev-list","--parents","-n","1","HEAD").decode().split()
    # Explicit raw parent is needed: a depth-2 checkout may place its parent on a shallow boundary.
    raw_commit=git("cat-file","-p","HEAD").decode()
    if raw_commit.split("\n\n",1)[0].splitlines().count("parent "+BASE)!=1:
        raise ValueError("review parent is absent")
    changes=git("diff","--no-renames","--name-only","-z",BASE,"HEAD","--").split(b"\0")[:-1]
    check_checkout(head,parents,[name.decode() for name in changes])
    print(f"Exact approved product {BASE}; smoke-only reviewed child {head}")


if __name__=="__main__":
    try:
        main()
    except (ValueError,KeyError,TypeError,AttributeError,OSError,subprocess.CalledProcessError):
        sys.exit("Rejected: native desktop QA event/source does not match the explicitly reviewed scope.")
