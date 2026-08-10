#!/usr/bin/env python3
"""Turn the changelog's Unreleased section into a released one, and hand its
bullets to the release.

Run by the build workflow at the moment it bumps the version, so every GitHub
release says what changed in it instead of "see the assets below" — the menu-bar
version item opens that page, and a person deciding whether to install an update
reads it there.

    python3 .github/scripts/cut_release_notes.py 0.1.45 > notes.md

Two audiences, one file. The Unreleased section is written as top-level bullets,
one line each, in plain language — those are what the release page shows.
Anything indented under a bullet is the engineering detail (why, what broke, what
is pinned by a test); it stays in the changelog and is left out of the release,
which is a list, not an essay.

Rewrites CHANGELOG.md in place (Unreleased becomes `## v0.1.45 — 2026-08-10`,
with a fresh empty Unreleased above it) and prints the bullets. An empty section
leaves the file untouched and prints nothing, so a release with nothing to say
falls back to the generic line. A section with prose but no bullets is an error:
silently publishing nothing is how the release page went stale in the first place.
"""

import datetime
import pathlib
import re
import sys

CHANGELOG = pathlib.Path(__file__).resolve().parents[2] / "CHANGELOG.md"
HEADING = "## Unreleased"


def collect_bullets(body: str) -> list[str]:
    """The section's top-level bullets, each rejoined into one line.

    A bullet ends at the first blank line or the first indented sub-bullet,
    whichever comes first. Only its own wrapped continuation lines are kept:
    an indented list and an indented paragraph are both engineering detail and
    belong to the file, not to the release page — glue either onto the bullet
    and the release turns into the wall of text this cutter exists to prevent.
    """
    bullets: list[str] = []
    current: list[str] = []

    def close() -> None:
        nonlocal current
        if current:
            bullets.append(" ".join(current))
            current = []

    for line in body.splitlines():
        indented_bullet = line[:1].isspace() and line.lstrip().startswith(("- ", "* "))
        if line.startswith("- "):
            close()
            current = [line.strip()]
        elif indented_bullet or not line.strip():
            close()
        elif current:
            current.append(line.strip())
    close()
    return bullets


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: cut_release_notes.py <version>", file=sys.stderr)
        return 2
    version = sys.argv[1]

    text = CHANGELOG.read_text()
    start = text.find(HEADING)
    if start == -1:
        print(f"{CHANGELOG.name} has no '{HEADING}' section", file=sys.stderr)
        return 1

    body_start = start + len(HEADING)
    # The section runs to the next second-level heading, or to the end of file.
    next_heading = re.search(r"^## ", text[body_start:], flags=re.MULTILINE)
    body_end = body_start + next_heading.start() if next_heading else len(text)
    body = text[body_start:body_end].strip()

    if not body:
        return 0

    bullets = collect_bullets(body)
    if not bullets:
        print(
            f"{CHANGELOG.name}: the Unreleased section has text but no top-level "
            "bullets — the release page would say nothing about this build",
            file=sys.stderr,
        )
        return 1

    today = datetime.date.today().isoformat()
    released = f"## v{version} — {today}\n\n{body}\n\n"
    CHANGELOG.write_text(text[:start] + f"{HEADING}\n\n" + released + text[body_end:].lstrip("\n"))
    print("\n".join(bullets))
    return 0


if __name__ == "__main__":
    sys.exit(main())
