#!/usr/bin/env python3
"""Verify the Windows bundle DLL closure by parsing PE import tables.

Every non-system import of every PE in `bin/` must be shipped in the bundle,
every library recorded in `toolchain-descriptor.json` must exist and be
imported by at least one PE, and no unlisted DLL may be present.
"""

import argparse
import json
import os
import struct
import sys

SYSTEM_DLLS = {
    "advapi32.dll",
    "avicap32.dll",
    "avrt.dll",
    "bcrypt.dll",
    "bcryptprimitives.dll",
    "cabinet.dll",
    "credui.dll",
    "d2d1.dll",
    "d3d12.dll",
    "dnsapi.dll",
    "dwrite.dll",
    "gdiplus.dll",
    "hid.dll",
    "mpr.dll",
    "msacm32.dll",
    "ncrypt.dll",
    "nsi.dll",
    "rasapi32.dll",
    "schannel.dll",
    "sechost.dll",
    "urlmon.dll",
    "usp10.dll",
    "winspool.drv",
    "wintrust.dll",
    "cfgmgr32.dll",
    "combase.dll",
    "comdlg32.dll",
    "crypt32.dll",
    "d3d11.dll",
    "d3d9.dll",
    "dbghelp.dll",
    "dwmapi.dll",
    "dxgi.dll",
    "gdi32.dll",
    "imm32.dll",
    "iphlpapi.dll",
    "kernel32.dll",
    "kernelbase.dll",
    "mf.dll",
    "mfplat.dll",
    "mfreadwrite.dll",
    "mfuuid.dll",
    "msimg32.dll",
    "msvcrt.dll",
    "netapi32.dll",
    "normaliz.dll",
    "ntdll.dll",
    "ole32.dll",
    "oleacc.dll",
    "oleaut32.dll",
    "opengl32.dll",
    "powrprof.dll",
    "psapi.dll",
    "rpcrt4.dll",
    "secur32.dll",
    "setupapi.dll",
    "shell32.dll",
    "shlwapi.dll",
    "strmiids.dll",
    "ucrtbase.dll",
    "user32.dll",
    "userenv.dll",
    "uxtheme.dll",
    "version.dll",
    "winhttp.dll",
    "wininet.dll",
    "winmm.dll",
    "wldap32.dll",
    "ws2_32.dll",
    "wtsapi32.dll",
}


def is_system_dll(name):
    lowered = name.lower()
    return (
        lowered in SYSTEM_DLLS
        or lowered.startswith("api-ms-win-")
        or lowered.startswith("ext-ms-")
    )


def read_c_string(data, offset):
    end = data.find(b"\x00", offset)
    if end < 0:
        raise ValueError("unterminated string")
    return data[offset:end].decode("ascii", errors="replace")


def rva_to_offset(sections, rva):
    for virtual_address, virtual_size, raw_size, raw_pointer in sections:
        if virtual_address <= rva < virtual_address + max(virtual_size, raw_size):
            return raw_pointer + (rva - virtual_address)
    raise ValueError(f"RVA {rva:#x} is not mapped")


def parse_sections(data, coff_offset, optional_size):
    count = struct.unpack_from("<H", data, coff_offset + 2)[0]
    sections = []
    offset = coff_offset + 20 + optional_size
    for _ in range(count):
        virtual_size, virtual_address, raw_size, raw_pointer = struct.unpack_from(
            "<IIII", data, offset + 8
        )
        sections.append((virtual_address, virtual_size, raw_size, raw_pointer))
        offset += 40
    return sections


def parse_imports(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data[:2] != b"MZ":
        raise ValueError("not a PE file")
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_offset : pe_offset + 4] != b"PE\x00\x00":
        raise ValueError("missing PE signature")
    magic = struct.unpack_from("<H", data, pe_offset + 24)[0]
    if magic == 0x10B:
        directory_offset = pe_offset + 24 + 96
    elif magic == 0x20B:
        directory_offset = pe_offset + 24 + 112
    else:
        raise ValueError(f"unsupported optional header magic {magic:#x}")
    optional_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
    sections = parse_sections(data, pe_offset + 4, optional_size)
    import_rva, import_size = struct.unpack_from("<II", data, directory_offset + 8)
    if import_rva == 0 or import_size == 0:
        return []
    offset = rva_to_offset(sections, import_rva)
    imports = []
    while True:
        entry = struct.unpack_from("<IIIII", data, offset)
        if entry == (0, 0, 0, 0, 0):
            break
        name_rva = entry[3]
        if name_rva:
            name_offset = rva_to_offset(sections, name_rva)
            imports.append(read_c_string(data, name_offset))
        offset += 20
    return imports


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle_root")
    args = parser.parse_args()

    bin_dir = os.path.join(args.bundle_root, "bin")
    descriptor_path = os.path.join(args.bundle_root, "toolchain-descriptor.json")
    with open(descriptor_path, encoding="utf-8") as handle:
        descriptor = json.load(handle)
    expected = {os.path.basename(entry["path"]).lower() for entry in descriptor["sharedLibraries"]}

    present = {
        name.lower()
        for name in os.listdir(bin_dir)
        if name.lower().endswith(".dll")
    }
    binaries = sorted(
        name
        for name in os.listdir(bin_dir)
        if name.lower().endswith((".exe", ".dll"))
    )

    errors = []
    imported_bundled = set()
    for name in binaries:
        imports = parse_imports(os.path.join(bin_dir, name))
        for imported in imports:
            lowered = imported.lower()
            if is_system_dll(lowered):
                continue
            if lowered not in present:
                errors.append(f"{name} imports {imported} which is not in the bundle")
            else:
                imported_bundled.add(lowered)

    for library in sorted(expected):
        if library not in present:
            errors.append(f"{library} is recorded in the descriptor but missing from bin/")
        elif library not in imported_bundled:
            errors.append(f"{library} is bundled but no PE imports it")

    for library in sorted(present - expected):
        errors.append(f"{library} is present but not recorded in the descriptor")

    for name in binaries:
        print(f"scanned {name}: {len(parse_imports(os.path.join(bin_dir, name)))} imports")

    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(
        f"PE closure verified: {len(binaries)} binaries, "
        f"{len(expected)} bundled libraries, no missing or unrecorded DLLs"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
