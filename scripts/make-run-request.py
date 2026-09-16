#!/usr/bin/env python3
"""Print a Protocol 0.1 StartRequest for a staged `input/source.media` file.

Usage: make-run-request.py <probe|extract_preview> <media-path> <toolchain-fingerprint>

The media file must already be staged as `input/source.media` inside the
staging root passed to `scene-core run`.
"""

import hashlib
import json
import os
import sys


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def digest(text):
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()


def hash_file(path):
    """Streams the file so large media never has to fit in memory."""
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return "sha256:" + hasher.hexdigest()


def main():
    operation, media, fingerprint = sys.argv[1], sys.argv[2], sys.argv[3]
    size = os.path.getsize(media)
    content = hash_file(media)
    input_fingerprint = digest(canonical({
        "inputSetVersion": "1",
        "inputs": [{"role": "source_media", "contentHash": content, "byteSize": size}],
    }))
    contract = "probe-result/1" if operation == "probe" else "extract-preview-result/1"
    derivation_key = digest(canonical({
        "derivationDescriptorVersion": "1",
        "engineCacheCompatibilityId": "scene-core-output-v1",
        "inputFingerprint": input_fingerprint,
        "operation": operation,
        "operationConfigHash": digest("{}"),
        "outputContractVersion": contract,
        "toolchainFingerprint": fingerprint,
    }))
    print(json.dumps({
        "engineProtocolVersion": "0.1",
        "messageType": "start",
        "requestId": "conformance_01",
        "operation": operation,
        "sourceVersionId": "sourcev_conformance",
        "inputs": [{
            "role": "source_media",
            "ref": "input/source.media",
            "contentHash": content,
            "byteSize": size,
        }],
        "inputFingerprint": input_fingerprint,
        "operationConfigHash": digest("{}"),
        "outputContractVersion": contract,
        "derivationKey": derivation_key,
        "executionContext": {
            "runId": "run_conformance",
            "generation": 0,
            "attempt": 1,
            "scope": "asset",
        },
        "deadlineMs": 300000,
        "options": {},
    }))


if __name__ == "__main__":
    main()
