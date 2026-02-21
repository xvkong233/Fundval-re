#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path


def extract_section(text: str, version: str) -> str | None:
    # 支持：
    # - ## [1.4.0] - 2026-02-21
    # - ## 1.4.0
    header = re.compile(rf"^##\s+(\[{re.escape(version)}\]|{re.escape(version)})(\s|$).*$", re.M)
    m = header.search(text)
    if not m:
        return None

    start = m.start()
    rest = text[m.end() :]
    next_header = re.search(r"^##\s+\[?\d+\.\d+\.\d+[^\]]*\]?.*$", rest, re.M)
    end = m.end() + (next_header.start() if next_header else len(rest))
    return text[start:end].strip() + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", required=True)
    ap.add_argument("--input", required=True)
    ap.add_argument("--output", required=True)
    args = ap.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)
    text = input_path.read_text(encoding="utf-8")

    section = extract_section(text, args.version)
    if not section:
        # 兜底：至少给出一个可用的 release body，避免 action 失败
        section = f"## {args.version}\n\n（未在 CHANGELOG.md 中找到对应版本段落）\n"

    output_path.write_text(section, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
