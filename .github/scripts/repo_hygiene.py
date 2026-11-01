#!/usr/bin/env python3
"""Repository hygiene gate derived from AGENTS.md (WP-P1).

Fails when tracked files include things the project must never publish
(archives, Java binaries, world/player saves, credentials, raw JVM
disassembly) or when the fixed planning checklist is structurally altered.
It never needs the proprietary Java server JAR and reads only tracked files.
Intentional exceptions go in .github/hygiene-allowlist.txt, one glob per
line followed by a reason; every exception is reviewed like any other change.
"""
import fnmatch
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path.cwd()

FORBIDDEN_EXTENSIONS = {
    ".jar", ".class", ".war", ".zip", ".tar", ".tgz", ".gz", ".bz2", ".xz",
    ".7z", ".rar", ".mca", ".mcr", ".dat", ".dat_old", ".lock_world",
}
FORBIDDEN_PATH_PARTS = ("playerdata/", "world/region/", "/region/r.", "session.lock")
FORBIDDEN_NAMES = {".env", "id_rsa", "id_ed25519"}
FORBIDDEN_SUFFIXES = (".pem", ".key", ".p12", ".pfx", ".keystore", ".jks")
SECRET_PATTERNS = {
    "GitHub token": re.compile(rb"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b|github_pat_[A-Za-z0-9_]{50,}"),
    "private key block": re.compile(rb"-----BEGIN (?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY-----"),
    "AWS access key id": re.compile(rb"\bAKIA[0-9A-Z]{16}\b"),
    "Slack token": re.compile(rb"\bxox[baprs]-[A-Za-z0-9-]{10,}\b"),
}
DISASSEMBLY_LINE = re.compile(
    rb"^\s+\d+: (?:[aildf](?:load|store)|invoke(?:virtual|static|special|interface|dynamic)|getfield|putfield|getstatic|putstatic|new|dup|[aildf]?return|ldc2?_?w?|checkcast|instanceof)(?:_\w+)?(?=\s|$)",
    re.MULTILINE,
)
DISASSEMBLY_THRESHOLD = 20  # a few quoted lines in prose are fine
MAX_SCAN_BYTES = 64 * 1024 * 1024
REQUIRED_FILES = ["LICENSE", "THIRD_PARTY_NOTICES.md", "AGENTS.md"]


def tracked_files():
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
    ).stdout
    return [p for p in out.decode().split("\0") if p]


def load_allowlist():
    path = ROOT / ".github" / "hygiene-allowlist.txt"
    patterns = []
    if path.exists():
        for line in path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                patterns.append(line.split()[0])
    return patterns


def allowed(path, patterns):
    return any(fnmatch.fnmatch(path, pattern) for pattern in patterns)


def check_files(files, allow, errors):
    for path in files:
        if allowed(path, allow):
            continue
        lower = path.lower()
        name = lower.rsplit("/", 1)[-1]
        suffix = "." + name.rsplit(".", 1)[-1] if "." in name else ""
        if suffix in FORBIDDEN_EXTENSIONS:
            errors.append(f"{path}: forbidden file type {suffix} (archives, Java binaries and saves are never committed)")
        if any(part in lower for part in FORBIDDEN_PATH_PARTS):
            errors.append(f"{path}: looks like a world/player save path")
        if name in FORBIDDEN_NAMES or lower.endswith(FORBIDDEN_SUFFIXES):
            errors.append(f"{path}: looks like a credential or key file")
        full = ROOT / path
        try:
            size = full.stat().st_size
        except OSError:
            continue
        if size > MAX_SCAN_BYTES:
            errors.append(f"{path}: {size} bytes exceeds the {MAX_SCAN_BYTES} byte repository scan limit")
            continue
        data = full.read_bytes()
        if b"\0" in data[:8192]:
            continue  # binary data is judged by type above
        for label, pattern in SECRET_PATTERNS.items():
            if pattern.search(data):
                errors.append(f"{path}: contains a {label}")
        if len(DISASSEMBLY_LINE.findall(data)) >= DISASSEMBLY_THRESHOLD:
            errors.append(f"{path}: looks like raw JVM disassembly (never committed)")


def check_required(errors):
    for name in REQUIRED_FILES:
        if not (ROOT / name).is_file():
            errors.append(f"{name}: required file is missing")
    licenses = ROOT / "licenses"
    if not licenses.is_dir() or not any(licenses.iterdir()):
        errors.append("licenses/: preserved third-party notices are missing")
    license_text = (ROOT / "LICENSE").read_text(errors="replace") if (ROOT / "LICENSE").is_file() else ""
    if "GNU GENERAL PUBLIC LICENSE" not in license_text.upper():
        errors.append("LICENSE: GPL-2.0-only text is missing")
    cargo = (ROOT / "Cargo.toml").read_text() if (ROOT / "Cargo.toml").is_file() else ""
    if 'license = "GPL-2.0-only"' not in cargo:
        errors.append('Cargo.toml: license must remain "GPL-2.0-only"')


def check_checklists(errors):
    for rel, rows_key in (("docs/coordination/checklist-v1.json", "rows"), ("progress/checklist.json", "rows")):
        path = ROOT / rel
        if not path.is_file():
            errors.append(f"{rel}: fixed checklist file is missing")
            continue
        try:
            doc = json.loads(path.read_text())
        except json.JSONDecodeError as error:
            errors.append(f"{rel}: invalid JSON: {error}")
            continue
        rows = doc.get(rows_key, [])
        if doc.get("scope_version") != 1:
            errors.append(f"{rel}: scope_version must stay 1 (a scope change must be deliberate and documented)")
        if doc.get("total_points") != 100 or len(rows) != 100:
            errors.append(f"{rel}: the 100-item denominator changed (total_points={doc.get('total_points')}, rows={len(rows)})")
        code = sum(1 for r in rows if r.get("code_ported"))
        main = sum(1 for r in rows if r.get("main_integrated"))
        if doc.get("code_ported_points") != code or doc.get("main_integrated_points") != main:
            errors.append(f"{rel}: summary counts ({doc.get('code_ported_points')}/{doc.get('main_integrated_points')}) disagree with rows ({code}/{main})")
        ids = [r.get("id") for r in rows]
        if len(set(ids)) != len(ids):
            errors.append(f"{rel}: duplicate row ids")
        if any(r.get("main_integrated") and not r.get("code_ported") for r in rows):
            errors.append(f"{rel}: a row is Main-integrated without being code-ported")


def main():
    errors = []
    files = tracked_files()
    check_files(files, load_allowlist(), errors)
    check_required(errors)
    check_checklists(errors)
    if errors:
        for message in errors:
            print(f"::error::{message}")
        print(f"repo hygiene: {len(errors)} problem(s)")
        return 1
    print(f"repo hygiene: ok ({len(files)} tracked files scanned)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
