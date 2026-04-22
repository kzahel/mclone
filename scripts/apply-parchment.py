#!/usr/bin/env python3
"""
Apply Parchment parameter-name mappings to an already-decompiled Minecraft
source tree. Rewrites `varN` identifiers in method signatures and bodies to
use the descriptive names Parchment provides.

What this fixes:
    public boolean carve(CarvingContext var1, CaveCarverConfiguration var2,
                         ChunkAccess var3, ..., BitSet var8) { ... var1.foo() ... }
becomes:
    public boolean carve(CarvingContext context, CaveCarverConfiguration config,
                         ChunkAccess chunk, ..., BitSet carvingMask) { ... context.foo() ... }

What this does NOT fix:
    True local variables (var9, var10, var11 ...) declared inside method bodies.
    Those names were never in the obfuscated jar; no mapping set can recover
    them.

Usage:
    apply-parchment.py SRC_DIR --mc 1.17.1 [--parchment-version X] [--cache DIR] [--dry-run]

The script is idempotent: running it twice is a no-op on the second pass
(method signatures no longer have varN params to match).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request
import zipfile
from collections import defaultdict
from pathlib import Path
from xml.etree import ElementTree as ET

PARCHMENT_MAVEN = "https://maven.parchmentmc.org/org/parchmentmc/data"

# Keywords that form `keyword(...)` { ... }` and must NOT be treated as method names.
CONTROL_KEYWORDS = {
    "if", "while", "for", "switch", "catch", "synchronized",
    "try", "do", "else", "return", "throw", "new",
}

# Matches a complete "(...) [throws ...] {" where the parens themselves don't
# contain nested parens. Good enough for Minecraft source; Java method params
# don't contain top-level parens outside of annotations-with-args, which are
# rare on method parameters in Minecraft.
SIG_RE = re.compile(
    r"\((?P<params>[^()]*)\)"
    r"(?P<throws>\s*throws\s+[\w.,\s<>]+)?"
    r"\s*\{",
    re.DOTALL,
)

VAR_RE = re.compile(r"\bvar\d+\b")


def fetch_latest_parchment_version(mc_version: str) -> str:
    url = f"{PARCHMENT_MAVEN}/parchment-{mc_version}/maven-metadata.xml"
    with urllib.request.urlopen(url) as r:
        root = ET.fromstring(r.read())
    latest = root.findtext("./versioning/release")
    if latest:
        return latest
    versions = [v.text for v in root.findall("./versioning/versions/version")]
    # Prefer non-SNAPSHOT if possible
    stable = [v for v in versions if v and "SNAPSHOT" not in v]
    if stable:
        return stable[-1]
    if versions:
        return versions[-1]
    raise RuntimeError(f"No Parchment versions for MC {mc_version}")


def download_parchment(mc_version: str, parchment_version: str, cache_dir: Path) -> Path:
    cache_dir.mkdir(parents=True, exist_ok=True)
    fname = f"parchment-{mc_version}-{parchment_version}.zip"
    dest = cache_dir / fname
    if dest.exists():
        return dest
    url = f"{PARCHMENT_MAVEN}/parchment-{mc_version}/{parchment_version}/{fname}"
    print(f"[parchment] Downloading {url}")
    urllib.request.urlretrieve(url, dest)
    return dest


def load_parchment_json(zip_path: Path) -> dict:
    with zipfile.ZipFile(zip_path) as z:
        name = next((n for n in z.namelist() if n.endswith("parchment.json")), None)
        if not name:
            raise RuntimeError(f"parchment.json not in {zip_path}")
        with z.open(name) as f:
            return json.load(f)


def index_parchment(data: dict) -> dict:
    """
    Returns: {class_fqn_slashes: [{"name": method_name, "param_names": [...]}]}
    Only methods with complete parameter-name data are kept.
    """
    idx: dict = defaultdict(list)
    for cls in data.get("classes", []):
        fqn = cls.get("name")
        if not fqn:
            continue
        for m in cls.get("methods", []):
            params = m.get("parameters") or []
            names = [p.get("name") for p in params]
            if not names or any(n is None for n in names):
                continue
            idx[fqn].append({
                "name": m.get("name", ""),
                "param_names": names,
            })
    return idx


def methods_for_file(class_index: dict, outer_fqn: str) -> dict:
    """
    Merge methods from the outer class and all its inner classes (by $-prefix).
    Returns: {method_name: [{"param_names": [...]}...]}
    Inner-class methods are merged because Vineflower inlines inner-class source
    into the outer .java file.
    """
    merged: dict = defaultdict(list)
    prefix = outer_fqn + "$"
    for fqn, methods in class_index.items():
        if fqn != outer_fqn and not fqn.startswith(prefix):
            continue
        for m in methods:
            merged[m["name"]].append(m)
    return merged


def split_params(params_str: str) -> list[str]:
    """Split on top-level commas; respect angle-bracket nesting in generics."""
    parts: list[str] = []
    depth = 0
    buf: list[str] = []
    for ch in params_str:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(buf).strip())
            buf.clear()
        else:
            buf.append(ch)
    if buf:
        parts.append("".join(buf).strip())
    return [p for p in parts if p]


def extract_var_name(param_decl: str) -> str | None:
    """From 'final @A Foo<Bar> var3' return 'var3'; else None."""
    m = VAR_RE.search(param_decl)
    return m.group(0) if m else None


def find_matching_brace(text: str, open_idx: int) -> int:
    """Return index just past the '}' matching text[open_idx]. Skips string/char literals and comments."""
    assert text[open_idx] == "{"
    i = open_idx + 1
    depth = 1
    n = len(text)
    while i < n and depth > 0:
        ch = text[i]
        if ch == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if ch == "'":
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == "'":
                    i += 1
                    break
                i += 1
            continue
        if ch == "/" and i + 1 < n:
            if text[i + 1] == "/":
                while i < n and text[i] != "\n":
                    i += 1
                continue
            if text[i + 1] == "*":
                i += 2
                while i + 1 < n and not (text[i] == "*" and text[i + 1] == "/"):
                    i += 1
                i += 2
                continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise RuntimeError("unmatched brace")


def identifier_before(text: str, idx: int) -> tuple[int, str]:
    """Walk backward from idx across whitespace, then capture the identifier. Returns (start, name) or (idx, '')."""
    j = idx
    while j > 0 and text[j - 1].isspace():
        j -= 1
    end = j
    while j > 0 and (text[j - 1].isalnum() or text[j - 1] == "_" or text[j - 1] == "$"):
        j -= 1
    return j, text[j:end]


def process_file(path: Path, src_root: Path, class_index: dict, stats: dict, dry_run: bool) -> None:
    rel = path.relative_to(src_root)
    parts = rel.with_suffix("").parts
    outer_fqn = "/".join(parts)

    lookup = methods_for_file(class_index, outer_fqn)
    if not lookup:
        stats["files_no_parchment"] += 1
        return

    text = path.read_text(encoding="utf-8")
    rewrites: list[tuple[int, int, dict[str, str]]] = []

    for sig in SIG_RE.finditer(text):
        params_str = sig.group("params").strip()
        if not params_str:
            continue

        params = split_params(params_str)
        var_names: list[str] | None = []
        for p in params:
            vn = extract_var_name(p)
            if vn is None:
                var_names = None
                break
            var_names.append(vn)
        if not var_names:
            continue

        # Walk back from '(' to grab the method name
        paren_open = sig.start()
        name_start, name = identifier_before(text, paren_open)
        if not name or name in CONTROL_KEYWORDS or not (name[0].isalpha() or name[0] == "_" or name[0] == "$"):
            continue

        # Bail out if this looks like an anonymous class body:
        # `new Foo(args) { ... }` — the '(' is preceded by `new`.
        # `identifier_before` would capture the class name; check what's before that.
        pre_idx, prior = identifier_before(text, name_start)
        if prior == "new":
            continue

        candidates = lookup.get(name, [])
        matches = [c for c in candidates if len(c["param_names"]) == len(var_names)]
        if len(matches) != 1:
            stats["ambiguous" if len(matches) > 1 else "unmatched_name"] += 1
            continue
        parchment_method = matches[0]

        renames: dict[str, str] = {}
        for vn, pname in zip(var_names, parchment_method["param_names"]):
            if vn != pname and pname:
                renames[vn] = pname
        if not renames:
            continue

        # Body brace position
        body_open = sig.end() - 1
        if text[body_open] != "{":
            continue
        try:
            body_close = find_matching_brace(text, body_open)
        except RuntimeError:
            continue

        # Apply rename across the full signature + body.
        rewrites.append((name_start, body_close, renames))
        stats["methods_renamed"] += 1

    if not rewrites:
        return

    # Sort by start descending so offsets stay valid as we splice.
    rewrites.sort(key=lambda r: r[0], reverse=True)
    for start, end, renames in rewrites:
        segment = text[start:end]
        def repl(m, rn=renames):
            return rn.get(m.group(0), m.group(0))
        new_segment = VAR_RE.sub(repl, segment)
        text = text[:start] + new_segment + text[end:]

    if dry_run:
        stats["files_would_update"] += 1
    else:
        path.write_text(text, encoding="utf-8")
        stats["files_updated"] += 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("src", type=Path, help="Decompiled source tree root (e.g. .../minecraft-1.17.1/src)")
    ap.add_argument("--mc", required=True, help="Minecraft version, e.g. 1.17.1")
    ap.add_argument("--parchment-version", help="Override Parchment version (default: latest release)")
    ap.add_argument("--cache", type=Path, help="Directory to cache Parchment zip (default: <src parent>/parchment)")
    ap.add_argument("--dry-run", action="store_true", help="Print stats without modifying files")
    args = ap.parse_args()

    if not args.src.is_dir():
        print(f"error: source tree not found: {args.src}", file=sys.stderr)
        return 2

    cache_dir = args.cache or (args.src.parent / "parchment")
    pv = args.parchment_version or fetch_latest_parchment_version(args.mc)
    print(f"[parchment] Using Parchment {pv} for MC {args.mc}")

    zip_path = download_parchment(args.mc, pv, cache_dir)
    data = load_parchment_json(zip_path)
    class_index = index_parchment(data)
    total_methods = sum(len(v) for v in class_index.values())
    print(f"[parchment] {total_methods} method entries across {len(class_index)} classes")

    java_files = list(args.src.rglob("*.java"))
    print(f"[parchment] Scanning {len(java_files)} .java files...")

    stats: dict = defaultdict(int)
    for jf in java_files:
        process_file(jf, args.src, class_index, stats, args.dry_run)

    print(f"[parchment] Methods renamed:     {stats['methods_renamed']}")
    print(f"[parchment] Files updated:       {stats['files_would_update' if args.dry_run else 'files_updated']}")
    print(f"[parchment] Files w/o Parchment: {stats['files_no_parchment']}")
    print(f"[parchment] Unmatched by name:   {stats['unmatched_name']}")
    print(f"[parchment] Ambiguous matches:   {stats['ambiguous']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
