"""Directives that read this repository's own sources at build time.

Reasoning: decision record 0036.

Nothing here parses Rust. ``cargo xtask book`` does that once and writes
``target/book/index.json``; this reads that. Two parsers of one thing disagree
about it eventually, and the whole point of assembling the book from checked
sources is to have fewer copies rather than more.

What this adds:

``nerpa:chapters``
    Expands into one page per section of ``docs/ARCHITECTURE.md``, in the order
    that file already puts them, carrying the section's own prose, its build
    marker, and the module documentation of everything that named its decision
    record with ``//! Expands: NNNN``.

``nerpa:invariant``
    Renders one invariant of the registry: what is guaranteed, how it could be
    bypassed, and whether anybody has attacked it. A page may assert a guarantee
    only through this directive, which is what keeps the book's promises and the
    registry's promises the same set.
"""

from __future__ import annotations

import json
from pathlib import Path

from docutils import nodes
from docutils.statemachine import StringList
from sphinx.util.docutils import SphinxDirective

# The book sits at docs/book, so the repository is two levels up.
ROOT = Path(__file__).resolve().parents[3]
INDEX = ROOT / "target" / "book" / "index.json"

#: How a section's build marker is said to a reader who is not reading the
#: source. The words are the same ones the architecture uses, because a second
#: vocabulary for one fact is a second thing to keep in step.
MARKERS = {
    "built": ("Built", "The code is here, and what follows describes it."),
    "partly built": (
        "Partly built",
        "Some of what follows exists and some does not. The text says which.",
    ),
    "planned": (
        "Planned",
        "Settled by its record and **not implemented**. Nothing below is a "
        "description of this program.",
    ),
}


def _index() -> dict:
    if not INDEX.exists():
        raise FileNotFoundError(
            f"{INDEX} is missing — run `cargo xtask book` before building the book"
        )
    return json.loads(INDEX.read_text(encoding="utf-8"))


def _registry() -> dict[str, dict]:
    return {entry["id"]: entry for entry in _index()["invariants"]}


def _slug(title: str) -> str:
    kept = [ch.lower() if ch.isalnum() else "-" for ch in title]
    return "".join(kept).strip("-").replace("--", "-")


def write_chapters(app):
    """Write one page per architecture section, before Sphinx looks for sources.

    This runs on ``builder-inited`` rather than inside the directive because
    Sphinx collects the source files once, at the start of the build. A page
    created while reading another page is a page the build never sees — which is
    exactly what happened the first time this was written the obvious way.
    """
    generated = Path(app.srcdir) / "chapters"
    generated.mkdir(exist_ok=True)

    written = set()
    for chapter in _index()["chapters"]:
        name = _slug(chapter["title"])
        written.add(f"{name}.md")
        generated.joinpath(f"{name}.md").write_text(
            _chapter_page(chapter), encoding="utf-8"
        )

    # A section that was renamed or removed leaves a page behind, and a stale
    # page is worse than a missing one: it describes something nobody declares.
    for stale in generated.glob("*.md"):
        if stale.name not in written:
            stale.unlink()


class Chapters(SphinxDirective):
    """The table of contents, in the order the architecture already puts it."""

    has_content = False

    def run(self):
        entries = [
            f"chapters/{_slug(chapter['title'])}" for chapter in _index()["chapters"]
        ]

        lines = [
            "```{toctree}",
            ":maxdepth: 2",
            ":caption: The shape of it",
            "",
            *entries,
            "```",
        ]
        node = nodes.section()
        node.document = self.state.document
        self.state.nested_parse(StringList(lines), self.content_offset, node)
        return node.children


def _chapter_page(chapter: dict) -> str:
    """One chapter, as MyST, from what the repository already says."""
    label, explanation = MARKERS.get(chapter["status"], ("Unmarked", ""))

    out = [f"# {chapter['title']}", ""]
    if chapter["record"]:
        out += [
            f"*Settled by decision record {chapter['record']}.*",
            "",
        ]
    out += [
        "```{admonition} " + label,
        ":class: " + ("note" if chapter["status"] == "built" else "warning"),
        "",
        explanation,
        "```",
        "",
        chapter["summary"],
        "",
    ]

    for module in chapter["modules"]:
        out += [
            "---",
            "",
            f"## `{module['path']}`",
            "",
            module["doc"],
            "",
        ]

    if not chapter["modules"]:
        out += [
            "```{note}",
            "No module has claimed this chapter yet. A module joins one by "
            "naming the same decision record in its own header:",
            "",
            "    //! Expands: " + (chapter["record"] or "NNNN"),
            "```",
            "",
        ]

    return "\n".join(out)


class Invariant(SphinxDirective):
    """One guarantee, as the registry states it and nowhere else."""

    required_arguments = 1
    has_content = False

    def run(self):
        wanted = self.arguments[0].strip()
        registry = _registry()
        if wanted not in registry:
            raise self.error(
                f"{wanted} is not in the registry — the book may not "
                f"promise what the registry does not"
            )

        entry = registry[wanted]
        status = entry["status"]
        shown = {
            "enforced": "enforced — somebody wrote the attempt and watched it refused",
            "claimed": "claimed — believed, not demonstrated",
            "planned": "planned — decided, not built",
        }.get(status, status)

        lines = [
            "```{admonition} " + f"{wanted} — {entry['statement']}",
            ":class: " + ("tip" if status == "enforced" else "caution"),
            "",
            f"**Status.** {shown}",
            "",
            entry.get("rationale", "").strip(),
        ]
        if entry.get("bypass") and entry["bypass"] != "unrepresentable":
            lines += ["", f"**How it could fail.** {entry['bypass']}"]
        elif entry.get("bypass") == "unrepresentable":
            lines += [
                "",
                "**How it could fail.** It cannot be written: the types make a "
                "violation impossible to express.",
            ]
        for test in entry.get("adversarial", []):
            lines += ["", f"*Attacked by* `{test}`"]
        lines.append("```")

        node = nodes.section()
        node.document = self.state.document
        self.state.nested_parse(StringList(lines), self.content_offset, node)
        return node.children


def setup(app):
    app.connect("builder-inited", write_chapters)
    app.add_directive("nerpa-chapters", Chapters)
    app.add_directive("nerpa-invariant", Invariant)
    return {"parallel_read_safe": True, "version": "0.1"}
