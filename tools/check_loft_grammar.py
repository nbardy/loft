# /// script
# dependencies = ["lark==1.3.1", "llguidance==1.9.1", "tiktoken"]
# ///
"""Independent acceptance test of the decoding grammar, with the two lexer rules in §06.A.4."""
from functools import lru_cache
from pathlib import Path
import re
import sys
import subprocess
import tiktoken
from lark import Lark, UnexpectedInput
from llguidance import LLMatcher, LLParserLimits, LLTokenizer, TokenizerWrapper

ROOT = Path(__file__).resolve().parents[1]
GRAMMAR_TEXT = (ROOT / "docs/spec/loft.lark").read_text()
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


# The generated corpus ends each build with hundreds of literal emit statements.
# Recognize that repeated nonterminal without constructing one enormous Earley forest.
# These are sufficient productions, so an unfamiliar grammar or row takes the full parser.
def literal_rows():
    productions = (
        'block:       "{" _NL* (stmt (_sep stmt)* _sep?)? "}"',
        '| "emit" kind_ref ("[" expr_list "]")? "(" named_args? ")"',
        'kind_ref: UIDENT | qualified',
        'named_args:  field_init ("," field_init)* ","?',
        'field_init:  LIDENT ":" expr',
        'unary_expr:  ("-" | "!") unary_expr | postfix_expr',
        'expr_list:   expr ("," expr)* ","?',
    )
    if not all(rule in GRAMMAR_TEXT for rule in productions):
        return None
    # Establish a complete derivation for the scalar, vector and call forms.
    GRAMMAR.parse("build { emit Sweep(paint: [0.1, -0.2, 3], height: -1,); emit Sweep(x: 0) }")
    def terminal(name):
        return "(?:" + GRAMMAR.get_terminal(name).pattern.to_regexp() + ")"
    num = "-?" + terminal("NUMBER")
    value = rf"(?:{num}|\[{num}, {num}, {num}\])"
    arg = rf"    {terminal('LIDENT')}: {value},\n"
    return re.compile(rf"  emit {terminal('UIDENT')}\(\n(?:{arg})+  \)\n(?:  \#[^\n]*\n)*")


LITERAL_ROW = literal_rows()


def earley_input(raw):
    # Compress only consecutive, whole emit statements ending the generated build.
    build = raw.find("\nbuild {\n")
    first = raw.find("  emit Sweep", build) if build >= 0 else -1
    if LITERAL_ROW is not None and first >= 0 and raw[first-1] == "\n":
        pos = first
        while match := LITERAL_ROW.match(raw, pos):
            pos = match.end()
        if pos > first and raw[pos:] == "}\n":
            return lexical_input(raw[:first] + "  emit Sweep(x: 0)\n}\n")
    return lexical_input(raw)


@lru_cache(maxsize=64)
def earley_accepts(source):
    try:
        GRAMMAR.parse(source)
        return True
    except UnexpectedInput:
        return False


def accepts(raw, generated=False):
    source = lexical_input(raw)
    earley = earley_accepts(earley_input(raw) if generated else source)
    decoder = LLMatcher(TOKENIZER, DECODER_GRAMMAR, limits=DECODER_LIMITS)
    if decoder.is_error():
        raise ValueError(decoder.get_error())
    tokens = TOKENIZER.tokenize_str(source.lstrip(" \t"))
    for start in range(0, len(tokens), 4096):
        if not decoder.consume_tokens(tokens[start:start + 4096]):
            return earley, False, start
    return earley, decoder.is_accepting() and not decoder.is_error(), None


def main():
    files = list(map(Path, sys.argv[1:]))
    fixtures = ROOT / "crates/loft-parse/tests/fixtures"
    if not files:
        names = subprocess.check_output(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.loft"], cwd=ROOT)
        files = sorted({ROOT / name.decode() for name in names.split(b"\0") if name})
        files.append(fixtures / "review-reject.invalid")
    failed = 0
    for path in files:
        expected = path.suffix != ".invalid"
        generated = path.resolve().is_relative_to(ROOT / "corpus/l5e") or path.resolve().is_relative_to(ROOT / "corpus/seeds")
        earley, decoder, rejected = accepts(path.read_text(), generated)
        if (earley, decoder) != (expected, expected):
            failed += 1
            print(f"{path}: wanted {expected}, Earley={earley}, llguidance={decoder} (failing chunk starts at byte {rejected})")
    card = (ROOT / "AGENT_CARD.md").read_text()
    tokens = len(tiktoken.get_encoding("o200k_base").encode_ordinary(card))
    if tokens >= 1400:
        raise ValueError(f"author card has {tokens} tokens (limit <1400)")
    for block in re.findall(r"```loft\n(.*?)\n```", card, re.S):
        if accepts(block)[:2] != (True, True):
            raise ValueError("author card failed the decoding grammar")
    print(f"{len(files)-failed}/{len(files)} files gave their expected result in Earley and llguidance")
    return bool(failed)


if __name__ == "__main__":
    sys.exit(main())
