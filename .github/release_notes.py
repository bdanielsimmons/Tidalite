"""Release notes from the commits since the last release, and the README's "What's new" list.

Usage: release_notes.py <new tag> <previous tag or ""> <notes file to write>

Every commit since the previous release becomes a line in the notes (its subject, plus any "- " points
in its body). A body line starting with "New:" marks a big feature: it is listed first in the notes and
added to the top of the README's What's new list (the last 8 are kept).
"""

import re
import subprocess
import sys

tag, prev, notes_path = sys.argv[1], sys.argv[2], sys.argv[3]
log = subprocess.run(
    ["git", "log", "--no-merges", "--format=%x1e%s%x1f%b", f"{prev}..HEAD" if prev else "HEAD"],
    capture_output=True, text=True, check=True,
).stdout

changes, new = [], []
for entry in log.split("\x1e")[1:]:
    subject, _, body = entry.partition("\x1f")
    subject = re.sub(r"\s*\[no release\]$", "", subject.strip())
    if subject.startswith("README: what's new"):
        continue
    lines = [f"- **{subject}**"]
    for line in body.splitlines():
        s = line.strip()
        if not s or s.lower().startswith("co-authored-by"):
            continue
        if s.lower().startswith("new:"):
            new.append(s[4:].strip())
        elif s.startswith("- ") or len(lines) == 1:
            lines.append("  " + (s if s.startswith("- ") else "- " + s))
        else:
            lines[-1] += " " + s  # a wrapped line continues the point above it
    changes.append("\n".join(lines))

out = []
if new:
    out += ["## New", *[f"- {n}" for n in new], ""]
out += ["## Changes", *(changes or ["- Rebuild, no code changes"]), ""]
out += [
    "## Downloads",
    "- **Windows:** `tidalite-practice.exe` (everything) or `tidalite-simple.exe` (no practice tools)",
    "- **Mac** (Apple Silicon and Intel): the `.tar.gz` files. Unpack, then right-click the app and choose Open"
    " the first time (it isn't signed).",
]
with open(notes_path, "w", encoding="utf-8") as f:
    f.write("\n".join(out) + "\n")

if new:
    with open("README.md", encoding="utf-8") as f:
        readme = f.read()
    m = re.search(r"<!-- whats-new -->\r?\n(.*?)<!-- /whats-new -->", readme, re.S)
    if m:
        old = [l for l in m.group(1).splitlines() if l.startswith("- ")]
        items = [f"- **{tag}:** {n}" for n in new] + old
        readme = readme[: m.start(1)] + "\n".join(items[:8]) + "\n" + readme[m.end(1) :]
        with open("README.md", "w", encoding="utf-8") as f:
            f.write(readme)
