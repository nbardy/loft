# /// script
# dependencies = ["lark==1.3.1", "llguidance==1.9.1"]
# ///
"""Independent acceptance test of the decoding grammar, with the two lexer rules in §06.A.4."""
from pathlib import Path
import re
import sys
from lark import Lark, UnexpectedInput
from llguidance import LLMatcher, LLParserLimits, LLTokenizer, TokenizerWrapper

ROOT = Path(__file__).resolve().parents[1]
GRAMMAR_TEXT = (ROOT / "grammar/loft.lark").read_text()
GRAMMAR = Lark(GRAMMAR_TEXT, parser="earley")


class ByteVocabulary:
    # Byte-complete vocabulary makes this probe independent of tokenizer downloads.
    tokens = [bytes([i]) for i in range(256)] + [b"<eos>"]
    eos_token_id = 256
    bos_token_id = None
    special_token_ids = [eos_token_id]

    def __call__(self, data):
        return list(data)


TOKENIZER = LLTokenizer(TokenizerWrapper(ByteVocabulary()))
DECODER_GRAMMAR = LLMatcher.grammar_from_lark(GRAMMAR_TEXT)
DECODER_LIMITS = LLParserLimits(step_max_items=1_000_000, verbose_errors=False)
TOKEN = re.compile(r'"(?:[^"\\]|\\.)*"|\#[^\n]*|\r?\n|[ \t]+|[\w]+|::|\|>|==|!=|<=|>=|&&|\|\||->|\.\.|\S')
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
            result.append(" .. " if t == ".." else t)
    return "".join(result)


def accepts(source):
    source = lexical_input(source)
    try:
        GRAMMAR.parse(source)
        earley = True
    except UnexpectedInput:
        earley = False
    decoder = LLMatcher(TOKENIZER, DECODER_GRAMMAR, limits=DECODER_LIMITS)
    if decoder.is_error():
        raise ValueError(decoder.get_error())
    tokens = TOKENIZER.tokenize_str(source.lstrip(" \t"))
    rejected = next((i for i, token in enumerate(tokens) if not decoder.consume_token(token)), None)
    return earley, rejected is None and decoder.is_accepting(), rejected


def main():
    files = list(map(Path, sys.argv[1:]))
    fixtures = ROOT / "crates/loft-parse/tests/fixtures"
    if not files:
        files = sorted(
            f for folder in ("lib", "engines", "agent_notes/2026-11-20_meta/v1_syntax")
            for f in (ROOT / folder).rglob("*.loft")
        ) + [fixtures / "review-accept.loft", fixtures / "review-reject.loft"]
    failed = 0
    for path in files:
        expected = path.name != "review-reject.loft"
        earley, decoder, rejected = accepts(path.read_text())
        if (earley, decoder) != (expected, expected):
            failed += 1
            print(f"{path}: wanted {expected}, Earley={earley}, llguidance={decoder} (byte {rejected})")
    print(f"{len(files)-failed}/{len(files)} files gave their expected result in Earley and llguidance")
    return bool(failed)


if __name__ == "__main__":
    sys.exit(main())
