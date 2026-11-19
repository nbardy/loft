# /// script
# dependencies = ["lark==1.3.1"]
# ///
"""Independent acceptance test of the decoding grammar, with the two lexer rules in §06.A.4."""
from pathlib import Path
import re
import sys
from lark import Lark, UnexpectedInput

ROOT = Path(__file__).resolve().parents[1]
GRAMMAR = Lark((ROOT / "grammar/loft.lark").read_text(), parser="earley")
TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"|\#[^\n]*|\r?\n|[ \t]+|[\w]+|::|\|>|==|!=|<=|>=|&&|\|\||->|\S')
CONTINUE = set("+ - * / % == != < <= > >= && || , = |>".split())


def lexical_input(source):
    # Lark consumes a single terminal for an island. Preserve character offsets for failures.
    tokens = []
    pos, raw = 0, False
    while pos < len(source):
        token = TOKEN.match(source, pos)
        word = token[0]
        pos = token.end()
        if raw and word == "{":
            level, comment = 1, 0
            while pos < len(source) and level:
                if source.startswith("/*", pos):
                    comment += 1
                    pos += 2
                elif comment and source.startswith("*/", pos):
                    comment -= 1
                    pos += 2
                elif comment:
                    pos += 1
                elif source.startswith("//", pos):
                    pos = source.find("\n", pos)
                    if pos < 0:
                        pos = len(source)
                elif source[pos] == '"':
                    quoted = TOKEN.match(source, pos)
                    pos = quoted.end()
                else:
                    level += int(source[pos] == "{") - int(source[pos] == "}")
                    pos += 1
            if level:
                raise ValueError("unterminated WGSL island")
            word = "{" + re.sub(r"[^\n]", " ", source[token.end():pos-1]) + "}"
            raw = False
        elif word == "wgsl":
            raw = True
        tokens.append(word)
    significant = [t for t in tokens if not t.isspace() and not t.startswith("#")]
    i, depth = 0, 0
    result = []
    for t in tokens:
        if t.startswith("#"):
            result.append(" " * len(t))
        elif t.isspace():
            prev = significant[i-1] if i else ""
            nxt = significant[i] if i < len(significant) else ""
            result.append(" " * len(t) if depth or (prev in CONTINUE and significant[max(0,i-2):i] != ["::", "*"]) or nxt in {"|>", "else"} else t)
        else:
            depth += int(t in {"(", "["}) - int(t in {")", "]"})
            i += 1
            result.append(t)
    return "".join(result)


def main():
    files = list(map(Path, sys.argv[1:])) or sorted(
        f for folder in ("lib", "engines", "agent_notes/2026-11-20_meta/v1_syntax")
        for f in (ROOT / folder).rglob("*.loft"))
    failed = 0
    for path in files:
        source = lexical_input(path.read_text())
        try:
            GRAMMAR.parse(source)
        except UnexpectedInput as err:
            failed += 1
            print(f"{path}:{err.line}:{err.column}\n{err.get_context(source)}")
    print(f"{len(files)-failed}/{len(files)} files accepted")
    return bool(failed)


if __name__ == "__main__":
    sys.exit(main())
