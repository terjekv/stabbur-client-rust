#!/usr/bin/env python3
"""Normalize and validate the pinned Stabbur OpenAPI contract."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "openapi" / "openapi.json"
SNAPSHOT = ROOT / "openapi" / "operations.json"


def content_types(content: object) -> list[str]:
    if not isinstance(content, dict):
        return []
    return sorted(str(value) for value in content)


def normalize(document: dict[str, object], source_bytes: bytes) -> dict[str, object]:
    operations: list[dict[str, object]] = []
    paths = document.get("paths", {})
    if not isinstance(paths, dict):
        raise SystemExit("OpenAPI paths must be an object")
    for path, path_item in paths.items():
        if not isinstance(path_item, dict):
            continue
        for method, operation in path_item.items():
            if method == "parameters" or not isinstance(operation, dict):
                continue
            operation_id = operation.get("operationId")
            if not isinstance(operation_id, str) or not operation_id:
                raise SystemExit(f"{method.upper()} {path} has no operationId")
            request_body = operation.get("requestBody", {})
            request_content = (
                request_body.get("content", {}) if isinstance(request_body, dict) else {}
            )
            response_types: set[str] = set()
            responses = operation.get("responses", {})
            if isinstance(responses, dict):
                for response in responses.values():
                    if isinstance(response, dict):
                        response_types.update(content_types(response.get("content", {})))
            security = operation.get("security", document.get("security", []))
            operations.append(
                {
                    "method": method.upper(),
                    "operation_id": operation_id,
                    "path": path,
                    "request_content_types": content_types(request_content),
                    "response_content_types": sorted(response_types),
                    "security": security,
                }
            )
    operations.sort(key=lambda item: (str(item["path"]), str(item["method"])))
    info = document.get("info", {})
    version = info.get("version") if isinstance(info, dict) else None
    return {
        "openapi": document.get("openapi"),
        "server_version": version,
        "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "operations": operations,
    }


def main() -> None:
    command = sys.argv[1] if len(sys.argv) > 1 else "validate"
    source_bytes = SOURCE.read_bytes()
    document = json.loads(source_bytes)
    expected = normalize(document, source_bytes)
    rendered = json.dumps(expected, indent=2, sort_keys=True) + "\n"
    if command == "update":
        SNAPSHOT.write_text(rendered, encoding="utf-8")
        print(f"updated {SNAPSHOT.relative_to(ROOT)}")
        return
    if command != "validate":
        raise SystemExit("usage: openapi-contract.py [validate|update]")
    if not SNAPSHOT.exists() or SNAPSHOT.read_text(encoding="utf-8") != rendered:
        raise SystemExit("normalized OpenAPI snapshot is stale; run update")
    print("pinned OpenAPI document and normalized snapshot agree")


if __name__ == "__main__":
    main()
