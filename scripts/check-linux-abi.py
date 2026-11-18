import os
import re
import subprocess
import sys
from pathlib import Path


LIMITS = {"GLIBC": (2, 35), "GLIBCXX": (3, 4, 30)}


def incompatible(version_info):
    needs = False
    for line in version_info.splitlines():
        if line.startswith("Version "):
            needs = line.startswith("Version needs section")
        if not needs:
            continue
        match = re.search(r"\bName: (\S+)", line)
        if not match:
            continue
        name = match[1]
        if name == "GLIBC_ABI_DT_RELR":
            yield name
        numeric = re.fullmatch(r"(GLIBC|GLIBCXX)_([0-9.]+)", name)
        if numeric and tuple(map(int, numeric[2].split("."))) > LIMITS[numeric[1]]:
            yield name


def check(root):
    files = root.rglob("*") if root.is_dir() else [root]
    count = 0
    errors = []
    for path in files:
        if path.is_symlink() or not path.is_file():
            continue
        with path.open("rb") as stream:
            if stream.read(4) != b"\x7fELF":
                continue
        count += 1
        info = subprocess.check_output(
            ["readelf", "--wide", "--version-info", str(path)],
            env={**os.environ, "LC_ALL": "C"}, text=True,
        )
        rejected = sorted(set(incompatible(info)))
        if rejected:
            errors.append(f"{path}: requires {', '.join(rejected)}")
    if not count:
        errors.append(f"{root}: no ELF files found")
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"{root}: {count} ELF files checked against Ubuntu 22.04")


if __name__ == "__main__":
    if not sys.argv[1:]:
        raise SystemExit("Pass the package files or extracted directories to check")
    for arg in sys.argv[1:]:
        check(Path(arg))
