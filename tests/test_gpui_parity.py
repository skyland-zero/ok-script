"""Parity guards between the web frontend and the native GPUI client.

These tests fail when the two frontends drift apart: every endpoint the web
client uses must be reachable from the native client, the shared translation
catalog and icon set must stay identical, and every configuration field kind
must have a native control.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WEB_SRC = ROOT / "web_src" / "src"
GPUI_SRC = ROOT / "ok" / "ui" / "gpui" / "src"
GPUI_ASSETS = ROOT / "ok" / "ui" / "gpui" / "assets" / "icons" / "ok"

# Endpoints the native shell intentionally does not call.
NATIVE_ALLOWLIST = {
    "/api/ui/ready",  # pywebview-only handshake
    "/api/status",  # dead endpoint in the web client as well
    "/api/task-tabs",  # navigation carries the manifests instead
}

CONFIG_KINDS = [
    "boolean",
    "select",
    "multi_selection",
    "list",
    "multiline",
    "integer",
    "number",
    "text",
    "file",
]


def _web_endpoints() -> set[str]:
    text = (WEB_SRC / "api.ts").read_text(encoding="utf-8")
    paths = set()
    for match in re.finditer(r"`?(/api/[A-Za-z0-9_\-{}$/.]+)`?", text):
        path = match.group(1)
        path = re.sub(r"\$\{[^}]+\}", "{name}", path)
        path = path.rstrip("/")
        if not path:
            continue
        paths.add(path)
    return paths


def _native_endpoints() -> set[str]:
    paths = set()
    for source in GPUI_SRC.glob("*.rs"):
        text = source.read_text(encoding="utf-8")
        for match in re.finditer(r'"(/api/[^"]*)"', text):
            path = match.group(1)
            path = re.sub(r"\{[^}]*\}", "{name}", path)
            path = path.split("?")[0].rstrip("/")
            if not path:
                continue
            paths.add(path)
    return paths


def _endpoint_matches(web: str, native: str) -> bool:
    """Match a concrete web path against the native templates."""
    web_parts = web.strip("/").split("/")
    native_parts = native.strip("/").split("/")
    if len(web_parts) != len(native_parts):
        return False
    for left, right in zip(web_parts, native_parts):
        if right.startswith("{") or right == "{name}":
            continue
        if left != right:
            return False
    return True


def test_every_web_endpoint_is_reachable_from_the_native_client():
    web = {
        path for path in _web_endpoints()
        if path not in NATIVE_ALLOWLIST
        and not any(part.startswith("${") for part in path.split("/"))
    }
    native = _native_endpoints()
    missing = sorted(
        path for path in web
        if not any(_endpoint_matches(path, candidate) for candidate in native)
    )
    assert missing == [], f"native client is missing endpoints: {missing}"


def test_shared_translation_catalog_is_identical():
    web_catalog = json.loads(
        (WEB_SRC / "i18n" / "catalogs.json").read_text(encoding="utf-8")
    )
    gpui_catalog = json.loads(
        (ROOT / "ok" / "ui" / "gpui" / "i18n" / "catalogs.json").read_text(encoding="utf-8")
    )
    assert gpui_catalog == web_catalog
    assert len(web_catalog["en_US"]) >= 180


def test_every_native_icon_is_vendored():
    icons = (GPUI_SRC / "icons.rs").read_text(encoding="utf-8")
    files = set(re.findall(r'=> "([a-z0-9\-]+)"', icons))
    assert files, "no icons declared"
    missing = sorted(name for name in files if not (GPUI_ASSETS / f"{name}.svg").is_file())
    assert missing == [], f"missing vendored icons: {missing}"
    table = (GPUI_SRC / "icons_data.rs").read_text(encoding="utf-8")
    embedded = set(re.findall(r'"icons/ok/([a-z0-9\-]+\.svg)"', table))
    assert embedded == {f"{name}.svg" for name in files}


def test_every_config_field_kind_has_a_native_control():
    pages = (GPUI_SRC / "pages.rs").read_text(encoding="utf-8")
    missing = [kind for kind in CONFIG_KINDS if f'"{kind}"' not in pages]
    assert missing == [], f"no native control for config kinds: {missing}"


def test_web_frontend_icon_names_are_covered():
    """Every Fluent icon the web frontend imports must have a native glyph."""
    app = (WEB_SRC / "App.tsx").read_text(encoding="utf-8")
    used = set(re.findall(r"\b([A-Z][A-Za-z0-9]*20Regular)\b", app))
    assert used, "no Fluent icons found in App.tsx"
    icons = (GPUI_SRC / "icons.rs").read_text(encoding="utf-8")
    mapped = set(re.findall(r'=> "([A-Za-z0-9]+20Regular)"', icons))
    # Brand marks are inline SVGs in the web frontend (GithubMark, ...).
    missing = {
        name for name in used
        if name not in mapped and not name.endswith("Mark")
    }
    assert missing == set(), f"Fluent icons without a native glyph: {sorted(missing)}"


def test_every_native_glyph_declares_its_fluent_origin():
    icons = (GPUI_SRC / "icons.rs").read_text(encoding="utf-8")
    variants = set()
    declared = set()
    for line in icons.splitlines():
        stripped = line.strip()
        if line.startswith("    ") and not line.startswith("        ") and ' => "' in line:
            variants.add(stripped.split(" => ")[0])
        if stripped.startswith("Self::") and ("20Regular" in stripped or "Mark" in stripped):
            declared.add(stripped.split("::", 1)[1].split(" =>", 1)[0])
    assert variants, "no icon variants declared"
    assert variants == declared, sorted(variants ^ declared)
