#!/usr/bin/env python3
"""Render schemas/0.1/*.json into docs/specs/schema-0.1-reference.md.

The schemas are generated from the Rust DTOs by `schema_contract`; this script
only renders them for review. Run from the repository root after regenerating
schemas:
    python3 scripts/schema-reference.py
"""

import json
import os
import subprocess

SCHEMA_DIR = "schemas/0.1"
OUTPUT = "docs/specs/schema-0.1-reference.md"

ORDER = [
    ("control-message.json", "Host→engine 控制消息（StartRequest / CancelRequest 的 oneOf）"),
    ("engine-event.json", "engine→Host 事件 Envelope（含 operation result union）"),
    ("normalized-media.json", "归一化媒体元数据（probe / extract_preview 共用）"),
    ("artifact-manifest.json", "Artifact Manifest（extract_preview）"),
    ("temporal-segment.json", "Scene/Shot seam：候选时间分段"),
    ("input-set-descriptor.json", "逻辑输入集合描述符（inputFingerprint 的 canonical 源）"),
    ("derivation-descriptor.json", "缓存推导描述符（derivationKey 的 canonical 源）"),
    ("engine-identity.json", "共享引擎身份（事件 / Manifest / version / doctor 复用）"),
    ("toolchain-identity.json", "共享工具链身份（version / doctor / Manifest 指纹来源）"),
    ("version.json", "`version --json` 输出"),
    ("doctor.json", "`doctor --json` 输出（有序 checks）"),
    ("package-manifest.json", "bundle 包内 manifest（逐文件完整性）"),
    ("cli-error.json", "CLI 无法产生正常输出时的单个错误对象"),
]

TYPE_NAMES = {
    "string": "string",
    "integer": "int",
    "boolean": "bool",
    "object": "object",
    "array": "array",
    "null": "null",
}


def ref_name(prop):
    ref = prop["$ref"]
    return ref.rsplit("/", 1)[-1]


def type_cell(prop):
    parts = []
    for keyword in ("anyOf", "oneOf"):
        if keyword in prop:
            return " | ".join(type_cell(branch) for branch in prop[keyword])
    if "const" in prop:
        parts.append(f"const `{prop['const']}`")
    elif "enum" in prop:
        values = ", ".join(f"`{value}`" for value in prop["enum"])
        parts.append(f"enum {values}")
    elif "$ref" in prop:
        parts.append(f"→ `{ref_name(prop)}`")
    else:
        kind = prop.get("type", "any")
        if isinstance(kind, list):
            kind = " | ".join(TYPE_NAMES.get(item, item) for item in kind)
        else:
            kind = TYPE_NAMES.get(kind, kind)
        parts.append(kind)
        if prop.get("format"):
            parts.append(f"format={prop['format']}")
        if prop.get("pattern"):
            parts.append(f"pattern=`{prop['pattern']}`")
        for key, label in (
            ("minimum", "min"),
            ("maximum", "max"),
            ("minLength", "minLen"),
            ("maxLength", "maxLen"),
        ):
            if key in prop:
                parts.append(f"{label}={prop[key]}")
        items = prop.get("items")
        if isinstance(items, dict) and items:
            parts.append(f"items: {type_cell(items)}")
    return "<br>".join(parts) if len(parts) > 1 else parts[0]


def constraint_line(schema):
    parts = []
    if "const" in schema:
        parts.append(f"const `{schema['const']}`")
    if "enum" in schema:
        values = ", ".join(f"`{value}`" for value in schema["enum"])
        parts.append(f"enum {values}")
    if schema.get("format"):
        parts.append(f"format={schema['format']}")
    if schema.get("pattern"):
        parts.append(f"pattern=`{schema['pattern']}`")
    for key, label in (
        ("minimum", "min"),
        ("maximum", "max"),
        ("minLength", "minLen"),
        ("maxLength", "maxLen"),
    ):
        if key in schema:
            parts.append(f"{label}={schema[key]}")
    return "; ".join(parts)


def render_object_table(schema):
    required = set(schema.get("required", []))
    lines = ["| 字段 | 类型 / 约束 | 必填 |", "|---|---|---|"]
    for field, prop in schema.get("properties", {}).items():
        lines.append(f"| `{field}` | {type_cell(prop)} | {'是' if field in required else '否'} |")
    if schema.get("additionalProperties") is False:
        lines.append("")
        lines.append("未知字段拒绝（`additionalProperties: false`）。")
    return lines


def render_schema(name, schema):
    description = schema.get("description")
    prefix = f"**{name}**" + (f" — {description}" if description else "")
    if "$ref" in schema:
        return [f"{prefix} → `{ref_name(schema)}`"]
    for keyword in ("oneOf", "anyOf"):
        if keyword in schema:
            alternatives = []
            for entry in schema[keyword]:
                if "$ref" in entry:
                    alternatives.append(f"`{ref_name(entry)}`")
                else:
                    alternatives.append("`" + json.dumps(entry, ensure_ascii=False) + "`")
            return [f"{prefix} — {keyword}: " + ", ".join(alternatives)]
    kind = schema.get("type")
    if kind == "string" or "const" in schema or "enum" in schema:
        return [f"{prefix} — string: {constraint_line(schema)}"]
    if kind == "array":
        items = schema.get("items", {})
        return [f"{prefix} — array of {type_cell(items)}"]
    if not schema.get("properties") and kind in (None, "object"):
        return [f"{prefix} — object (no fields)"]
    lines = [prefix, ""]
    lines.extend(render_object_table(schema))
    return lines


def main():
    commit = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    lines = [
        "# Protocol 0.1 Schema 全量参考",
        "",
        "> 由 `schemas/0.1/*.json` 渲染（`scripts/schema-reference.py`）。Schema 本身由 "
        "Rust DTO 生成，`scripts/check-schema-drift.sh` 阻止漂移。",
        f"> 快照：`{commit}`（2026-09-16）。JSON Schema Draft 2020-12；所有对象默认拒绝未知字段。",
        "> `类型` 列中 `int | null` 表示未知值必须显式为 null；`format=uint64` 表示非负整数。",
        "",
    ]
    for filename, purpose in ORDER:
        path = os.path.join(SCHEMA_DIR, filename)
        with open(path, encoding="utf-8") as handle:
            schema = json.load(handle)
        title = schema.get("title", filename)
        lines.append(f"## `{filename}` — {purpose}")
        lines.append("")
        lines.extend(render_schema(title, schema))
        definitions = schema.get("$defs", {})
        if definitions:
            lines.append("")
            lines.append("### 共享定义（`$defs`）")
            for def_name, definition in definitions.items():
                lines.append("")
                lines.extend(render_schema(def_name, definition))
        lines.append("")
    with open(OUTPUT, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines).rstrip() + "\n")
    print(f"wrote {OUTPUT}: {len(lines)} lines")


if __name__ == "__main__":
    main()
