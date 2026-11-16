#!/usr/bin/env python3
"""Flag em dashes, semicolons and stock phrases in the project's prose.

Checks Markdown and plain-text docs (outside code fences and inline code) and the
user-facing string literals in the Rust sources (status/error text). Prints
file:line for every hit and exits 1 if anything is found.

  python scripts/build/prose_check.py            # whole project
  python scripts/build/prose_check.py docs       # one folder or file
"""
import pathlib, re, sys

PROJECT = pathlib.Path(__file__).resolve().parents[2]
SKIP_DIRS = {'target', '.git', 'evidence'}  # docs/evidence holds raw tool output
STOCK = re.compile(r'\b(delve|tapestry|testament|pivotal|crucial|robust|seamless(ly)?|landscape|realm|'
                   r'navigate|foster|underscore[sd]?|multifaceted|nuanced|plays a (vital|key) role|'
                   r'incredibly|truly remarkable|ultimately|in essence|at the end of the day)\b', re.I)


def prose_lines(path):
    """Yield (line_no, text) of prose, with code fences, inline code and URLs removed."""
    fenced = False
    for no, line in enumerate(path.read_text(encoding='utf-8', errors='replace').splitlines(), 1):
        if line.lstrip().startswith(('```', '~~~')):
            fenced = not fenced
            continue
        if fenced:  # every code sample in these docs is fenced, so indented lines are prose
            continue
        text = re.sub(r'`[^`]*`', '', line)
        text = re.sub(r'\]\([^)]*\)|https?://\S+|&[a-z]+;|&#\d+;', '', text)
        yield no, text


def rust_strings(path):
    """Yield (line_no, text) of string literals that read like messages (contain a space)."""
    for no, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        if line.lstrip().startswith('//'):
            continue
        for lit in re.findall(r'"((?:[^"\\]|\\.)*)"', line):
            if ' ' in lit and not lit.startswith(('{', 'http')):
                yield no, re.sub(r'\{[^}]*\}', '', lit)


def check(path):
    hits = []
    lines = rust_strings(path) if path.suffix == '.rs' else prose_lines(path)
    for no, text in lines:
        for label, found in (('em dash', '—' in text), ('semicolon', ';' in text),
                             ('stock phrase', STOCK.search(text))):
            if found:
                hits.append(f'{path.relative_to(PROJECT).as_posix()}:{no}: {label}: {text.strip()[:110]}')
    return hits


def main():
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    roots = [PROJECT / a for a in sys.argv[1:]] or [PROJECT]
    files = []
    for root in roots:
        candidates = [root] if root.is_file() else root.rglob('*')
        files += [f for f in candidates if f.is_file() and f.suffix in ('.md', '.txt', '.rs')
                  and not SKIP_DIRS & set(f.relative_to(PROJECT).parts) and f.name not in ('SHA256SUMS.txt',)]
    hits = [h for f in sorted(files) for h in check(f)]
    print('\n'.join(hits) if hits else 'no em dashes, semicolons or stock phrases in prose')
    print(f'{len(hits)} hit(s) in {len({h.split(":")[0] for h in hits})} file(s)', file=sys.stderr)
    return 1 if hits else 0


if __name__ == '__main__':
    sys.exit(main())
