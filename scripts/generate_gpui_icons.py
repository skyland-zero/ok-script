"""Regenerate the native client's Fluent icon set.

The GPUI shell draws the same Fluent System Icons (20 regular) the web
frontend imports from `@fluentui/react-icons`, so both UIs render identical
glyphs. This script extracts the SVG path data from the npm package (the same
version the web bundle uses) and writes:

* `ok/ui/gpui/assets/icons/ok/<name>.svg` — one standalone SVG per glyph
* `ok/ui/gpui/src/icons_data.rs` — the `include_bytes!` table the asset source serves

Usage (from the repository root):

```bash
npm pack @fluentui/react-icons            # or: npm i --no-save @fluentui/react-icons
tar xzf fluentui-react-icons-*.tgz
python scripts/generate_gpui_icons.py \
    --source package/lib/atoms/svg \
    --out ok/ui/gpui/assets/icons/ok \
    --table ok/ui/gpui/src/icons_data.rs
```

`tests/test_gpui_parity.py` verifies that every icon the web frontend uses has
a vendored native glyph and that the table matches the files on disk.
"""

from __future__ import annotations

import argparse
import json
import os
import re

# Native glyph name -> Fluent icon component name (`IconName` in icons.rs).
NEEDED = [
    ("add", "Add20Regular"),
    ("alert", "Alert20Regular"),
    ("arrow-down", "ArrowDown20Regular"),
    ("arrow-download", "ArrowDownload20Regular"),
    ("arrow-export", "ArrowExport20Regular"),
    ("arrow-import", "ArrowImport20Regular"),
    ("arrow-left", "ArrowLeft20Regular"),
    ("arrow-right", "ArrowRight20Regular"),
    ("arrow-sync", "ArrowSync20Regular"),
    ("arrow-up", "ArrowUp20Regular"),
    ("broom", "Broom20Regular"),
    ("calendar", "Calendar20Regular"),
    ("capture", "Camera20Regular"),
    ("checkmark-circle", "CheckmarkCircle20Regular"),
    ("chevron-down", "ChevronDown20Regular"),
    ("chevron-left", "ChevronLeft20Regular"),
    ("chevron-right", "ChevronRight20Regular"),
    ("chevron-up", "ChevronUp20Regular"),
    ("close", "Dismiss20Regular"),
    ("code", "Code20Regular"),
    ("copy", "Copy20Regular"),
    ("delete", "Delete20Regular"),
    ("developer-board", "DeveloperBoard20Regular"),
    ("document-text", "DocumentText20Regular"),
    ("edit", "Edit20Regular"),
    ("error-circle", "ErrorCircle20Regular"),
    ("eye", "Eye20Regular"),
    ("filter", "Filter20Regular"),
    ("folder", "Folder20Regular"),
    ("globe", "Globe20Regular"),
    ("grid", "Grid20Regular"),
    ("heart", "Heart20Regular"),
    ("history", "History20Regular"),
    ("image", "Image20Regular"),
    ("info", "Info20Regular"),
    ("lightbulb", "Lightbulb20Regular"),
    ("list", "List20Regular"),
    ("local-language", "LocalLanguage20Regular"),
    ("navigation", "Navigation20Regular"),
    ("paint-brush", "PaintBrush20Regular"),
    ("pause", "Pause20Regular"),
    ("person", "Person20Regular"),
    ("play", "Play20Regular"),
    ("question-circle", "QuestionCircle20Regular"),
    ("record", "Record20Regular"),
    ("refresh", "ArrowClockwise20Regular"),
    ("save", "Save20Regular"),
    ("search", "Search20Regular"),
    ("settings", "Settings20Regular"),
    ("share", "Share20Regular"),
    ("shield", "Shield20Regular"),
    ("square", "Square20Regular"),
    ("star", "Star20Regular"),
    ("stop", "Stop20Regular"),
    ("subtract", "Subtract20Regular"),
    ("task-list", "TaskListSquareLtr20Regular"),
    ("timer", "Timer20Regular"),
    ("weather-moon", "WeatherMoon20Regular"),
    ("window", "Window20Regular"),
]

BACKSLASH = chr(92)
QUOTES = ("'", '"')
SVG_TAGS = {
    "path", "g", "circle", "rect", "ellipse", "line", "polyline", "polygon",
    "defs", "linearGradient", "radialGradient", "stop", "clipPath", "mask",
    "use", "text", "tspan",
}
ATTR_MAP = {
    "fillRule": "fill-rule",
    "clipRule": "clip-rule",
    "fillOpacity": "fill-opacity",
    "strokeWidth": "stroke-width",
    "strokeLinecap": "stroke-linecap",
    "strokeLinejoin": "stroke-linejoin",
}


def icon_file(source: str, name: str) -> str:
    base = re.sub(r"(20|24|28|32|48)(Regular|Filled|Color|Light)$", "", name)
    kebab = re.sub(r"(?<!^)(?=[A-Z])", "-", base).lower()
    return os.path.join(source, kebab + ".js")


def balanced(text: str, start: int) -> str | None:
    depth = 0
    in_string = None
    index = start
    while index < len(text):
        char = text[index]
        if in_string:
            if char == BACKSLASH:
                index += 2
                continue
            if char == in_string:
                in_string = None
        else:
            if char in QUOTES:
                in_string = char
            elif char in "([{":
                depth += 1
            elif char in ")]}":
                depth -= 1
                if depth == 0:
                    return text[start:index + 1]
        index += 1
    return None


def parse_literal(literal: str):
    text = literal.replace("'", '"')
    text = re.sub(r",\s*\]", "]", text)
    try:
        return json.loads(text)
    except Exception:
        return None


def render(node) -> str:
    if isinstance(node, str):
        return '<path d="%s" fill="currentColor"/>' % node
    if isinstance(node, list) and node and isinstance(node[0], str) and node[0] in SVG_TAGS:
        tag = node[0]
        attrs = ""
        children = ""
        if len(node) > 1 and isinstance(node[1], dict):
            for key, value in node[1].items():
                if value is None:
                    continue
                attrs += ' %s="%s"' % (ATTR_MAP.get(key, key), value)
            rest = node[2:]
        else:
            rest = node[1:]
        for item in rest:
            children += render(item)
        if tag == "path":
            if "fill=" not in attrs:
                attrs += ' fill="currentColor"'
            return "<path%s/>" % attrs
        return "<%s%s>%s</%s>" % (tag, attrs, children, tag)
    if isinstance(node, list):
        return "".join(render(item) for item in node)
    return ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True,
                        help="path to `@fluentui/react-icons/lib/atoms/svg`")
    parser.add_argument("--out", required=True, help="output directory for the SVGs")
    parser.add_argument("--table", help="optional path to regenerate icons_data.rs")
    args = parser.parse_args()

    os.makedirs(args.out, exist_ok=True)
    missing = []
    written = []
    for glyph, icon in NEEDED:
        path = icon_file(args.source, icon)
        if not os.path.exists(path):
            missing.append(icon)
            continue
        text = open(path, encoding="utf-8").read()
        index = text.find("export const %s =" % icon)
        if index < 0:
            missing.append(icon)
            continue
        call = text.find("createFluentIcon(", index)
        literal = balanced(text, text.find("(", call) + 1)
        parts = literal.split(",", 2) if literal else []
        if len(parts) < 3:
            missing.append(icon)
            continue
        size = parts[1].strip().strip("'\"")
        data = parse_literal(parts[2].strip().rstrip(")"))
        if data is None:
            missing.append(icon)
            continue
        svg = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %s %s" width="%s" '
            'height="%s" fill="none">%s</svg>\n'
            % (size, size, size, size, render(data))
        )
        with open(os.path.join(args.out, glyph + ".svg"), "w", encoding="utf-8") as handle:
            handle.write(svg)
        written.append(glyph + ".svg")

    if args.table:
        lines = [
            "// @generated by scripts/generate_gpui_icons.py — do not edit by hand.",
            "",
            "/// Embedded Fluent System Icons (20 regular), identical to the set the web",
            "/// frontend renders via `@fluentui/react-icons`.",
            "pub static ICONS: &[(&str, &[u8])] = &[",
        ]
        for name in sorted(written):
            lines.append(
                '    ("icons/ok/%s", include_bytes!("../assets/icons/ok/%s")),' % (name, name)
            )
        lines.append("]")
        lines.append("")
        with open(args.table, "w", encoding="utf-8") as handle:
            handle.write("\n".join(lines))

    print("icons written: %d" % len(written))
    if missing:
        print("missing: %s" % ", ".join(missing))
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
