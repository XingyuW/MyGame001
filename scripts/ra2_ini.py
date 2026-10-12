#!/usr/bin/env python3
# Tolerant reader for the vanilla Red Alert 2 `rules.ini` and `art.ini`, the reference data of
# the RA2 parity programme (docs/DECISIONS.md, RA2 parity programme; docs/RA2_GAP_ANALYSIS.md).
#
# The ini files are reference data kept OUTSIDE the repository and are never committed: by
# default they are read from ~/Library/Caches/MyGame001/ra2-reference/, or from the directory
# named by the environment variable RA2_REFERENCE_DIR. Cite single values as
# `[Section] Key=value`; do not copy the files or large excerpts of them into the repository.
#
# Parsing rules: `;` starts a comment anywhere on a line; section and key names keep their
# case but are looked up case-insensitively; a repeated key keeps its last value; a repeated
# section is merged into the first; a key with a blank value is kept as an empty string; lines
# without `=` and lines before the first section are ignored.
#
# As a module:
#     import ra2_ini
#     rules = ra2_ini.load(ra2_ini.default_path("rules"))
#     ra2_ini.get(rules, "E1", "Cost")            # "200"
#     ra2_ini.get(rules, "E1", "NoSuchKey", "x")  # "x"
#     rules.section("e1")                         # {"UIName": "Name:E1", ...} or None
#
# As a command (standard library only, Python 3):
#     python3 scripts/ra2_ini.py rules <Section> [Key]
#     python3 scripts/ra2_ini.py art <Section> [Key]
# prints the `Key=value` pairs of the section, or the one value. Exit code 1 when the file,
# the section or the key is missing.
"""Tolerant reader for the Red Alert 2 reference ini files kept outside the repository."""

import os
import sys

DEFAULT_DIR = os.path.join("~", "Library", "Caches", "MyGame001", "ra2-reference")
FILES = {"rules": "rules.ini", "art": "art.ini"}


class Ini:
    """Parsed ini file: sections in file order, with case-insensitive lookup."""

    def __init__(self):
        # lower-case section name -> (name as written, {lower-case key -> (key, value)})
        self._sections = {}

    def _open_section(self, name):
        entry = self._sections.get(name.lower())
        if entry is None:
            entry = (name, {})
            self._sections[name.lower()] = entry
        return entry[1]

    def sections(self):
        """Section names as first written, in file order."""
        return [name for name, _ in self._sections.values()]

    def has_section(self, section):
        return section.lower() in self._sections

    def section_name(self, section):
        """The section name as written in the file, or None."""
        entry = self._sections.get(section.lower())
        return None if entry is None else entry[0]

    def section(self, section):
        """The section as a dict of key (as written) -> value, or None."""
        entry = self._sections.get(section.lower())
        if entry is None:
            return None
        return {key: value for key, value in entry[1].values()}

    def get(self, section, key, default=None):
        entry = self._sections.get(section.lower())
        if entry is None:
            return default
        pair = entry[1].get(key.lower())
        return default if pair is None else pair[1]


def reference_dir():
    """Directory holding the reference ini files."""
    return os.path.expanduser(os.environ.get("RA2_REFERENCE_DIR") or DEFAULT_DIR)


def default_path(which):
    """Default path of `rules` or `art`."""
    return os.path.join(reference_dir(), FILES[which])


def parse(text):
    """Parse ini text into an Ini."""
    ini = Ini()
    current = None
    for raw in text.splitlines():
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        if line.startswith("["):
            end = line.find("]")
            if end > 0:
                current = ini._open_section(line[1:end].strip())
            continue
        if current is None or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        if key:
            current[key.lower()] = (key, value.strip())
    return ini


def load(path=None):
    """Load an ini file; `path` defaults to the reference rules.ini."""
    if path is None:
        path = default_path("rules")
    with open(path, "r", encoding="latin-1", newline=None) as handle:
        return parse(handle.read())


def get(ini, section, key, default=None):
    """Value of `key` in `section`, both case-insensitive, or `default`."""
    return ini.get(section, key, default)


def main(argv):
    if len(argv) not in (3, 4) or argv[1] not in FILES:
        sys.stderr.write("usage: ra2_ini.py rules|art <Section> [Key]\n")
        return 2
    path = default_path(argv[1])
    try:
        ini = load(path)
    except OSError as error:
        sys.stderr.write("cannot read %s: %s\n" % (path, error))
        return 1
    values = ini.section(argv[2])
    if values is None:
        sys.stderr.write("no section [%s] in %s\n" % (argv[2], path))
        return 1
    if len(argv) == 4:
        value = ini.get(argv[2], argv[3])
        if value is None:
            sys.stderr.write("no key %s in [%s]\n" % (argv[3], ini.section_name(argv[2])))
            return 1
        print(value)
        return 0
    for key, value in values.items():
        print("%s=%s" % (key, value))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
