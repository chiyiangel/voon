#!/usr/bin/env python3
"""Read-only APE/CUE metadata diagnostics; Python standard library only."""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import struct
import sys

MAX_ITEMS = 256
MAX_TEXT = 65536
MAX_TOTAL_TEXT = 262144
MAX_CUE = 262144
ENCODINGS = ("utf-8", "gb18030", "big5", "shift_jis", "cp1251", "cp1252")


def quoted(value):
    # Escape control characters so malformed tags cannot control the terminal.
    return json.dumps(str(value), ensure_ascii=False)


def read_at(stream, offset, length):
    if offset < 0 or length < 0:
        raise ValueError("negative file range")
    stream.seek(offset)
    data = stream.read(length)
    if len(data) != length:
        raise ValueError("file ended before the declared range (or changed during reading)")
    return data


def describe_text(data, label, out, preview=1500):
    print(f"{label}: bytes={len(data)}, sha256={hashlib.sha256(data).hexdigest()}", file=out)
    print("  raw_base64=" + base64.b64encode(data).decode("ascii"), file=out)
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        start = max(0, error.start - 16)
        end = min(len(data), error.end + 16)
        print(f"  UTF-8 ERROR: offset={error.start}, end={error.end}, reason={error.reason}", file=out)
        print(f"  bytes[{start}:{end}]={data[start:end].hex(' ')}", file=out)
    else:
        print("  UTF-8 OK: " + quoted(text[:preview]), file=out)
        if len(text) > preview:
            print("  (text preview truncated; raw_base64 is complete)", file=out)
        return
    encodings = list(ENCODINGS[1:])
    if data.startswith((b'\xff\xfe', b'\xfe\xff')):
        encodings.insert(0, "utf-16")
    for encoding in encodings:
        try:
            text = data.decode(encoding, errors="strict")
        except UnicodeDecodeError as error:
            print(f"  candidate {encoding}: FAIL at byte {error.start}", file=out)
        else:
            print(f"  candidate {encoding}: " + quoted(text[:preview]), file=out)
    print("  Candidate success does NOT identify the actual encoding.", file=out)


def tag_items(stream, start, end, count, out):
    position = start
    budget = MAX_TOTAL_TEXT
    for number in range(min(count, MAX_ITEMS)):
        if position + 8 > end:
            raise ValueError(f"item {number}: item header exceeds tag boundary")
        length, flags = struct.unpack('<II', read_at(stream, position, 8))
        position += 8
        key_start = position
        key_buffer = read_at(stream, position, min(256, end - position))
        separator = key_buffer.find(b'\0')
        if separator < 0:
            raise ValueError(f"item {number}: no key terminator within 256 bytes")
        key = key_buffer[:separator]
        position += separator + 1
        kind = (flags >> 1) & 3
        if position + length > end:
            raise ValueError(f"item {number}: value exceeds tag boundary")
        name = key.decode('ascii', errors='backslashreplace')
        print(f"\nItem {number + 1}: key={quoted(name)}, key_offset={key_start}, value_offset={position}, bytes={length}, flags=0x{flags:08x}, type={kind}", file=out)
        print("  key_hex=" + key.hex(), file=out)
        try:
            key.decode('utf-8', errors='strict')
        except UnicodeDecodeError as error:
            print(f"  KEY UTF-8 ERROR at byte {error.start}", file=out)
        if kind in (0, 2):
            if length > MAX_TEXT or length > budget:
                print("  OMITTED: text exceeds diagnostic read budget", file=out)
            else:
                value = read_at(stream, position, length)
                budget -= length
                describe_text(value, "  text" if kind == 0 else "  external locator", out)
        elif kind == 1:
            print("  Binary payload omitted (including cover art).", file=out)
        else:
            print("  ERROR: reserved item type", file=out)
        position += length
    if count > MAX_ITEMS:
        print(f"STOP: item count {count} exceeds diagnostic limit {MAX_ITEMS}", file=out)
    elif position != end:
        print(f"WARNING: {end - position} trailing bytes inside declared tag payload", file=out)


def inspect_ape(path, out):
    print("\n=== APE FILE ===", file=out)
    print("name=" + quoted(path.name), file=out)
    size = path.stat().st_size
    print(f"file_bytes={size}", file=out)
    with path.open('rb') as stream:
        head = read_at(stream, 0, min(size, 16))
        print("first_16_bytes=" + head.hex(), file=out)
        audio_start = 0
        if head.startswith(b'ID3') and len(head) >= 10:
            if any(byte & 0x80 for byte in head[6:10]):
                print("WARNING: invalid ID3v2 synchsafe size", file=out)
            else:
                tag_size = sum(byte << shift for byte, shift in zip(head[6:10], (21, 14, 7, 0)))
                audio_start = 10 + tag_size + (10 if head[3] == 4 and head[5] & 0x10 else 0)
                print(f"leading_ID3v2: version=2.{head[3]}.{head[4]}, flags=0x{head[5]:02x}, declared_payload_bytes={tag_size}; payload not exported", file=out)
        if audio_start + 6 <= size:
            signature = read_at(stream, audio_start, 6)
            print(f"audio_header_offset={audio_start}, signature={signature.hex()}", file=out)
            if signature[:4] == b'MAC ':
                print(f"Monkey_Audio_version={struct.unpack('<H', signature[4:])[0]}", file=out)
        end = size
        if size >= 128 and read_at(stream, size - 128, 3) == b'TAG':
            id3 = read_at(stream, size - 128, 128)
            print("trailing_ID3v1=yes; raw_base64=" + base64.b64encode(id3).decode('ascii'), file=out)
            end -= 128
        else:
            print("trailing_ID3v1=no", file=out)
        # Standard APE tag footer at EOF, or immediately before ID3v1.
        if end < 32 or read_at(stream, end - 32, 8) != b'APETAGEX':
            print("No APE footer at standard positions. This does not prove the file has no tags; a nonstandard layout or another tag format needs further inspection.", file=out)
            return
        footer = read_at(stream, end - 32, 32)
        version, tag_size, count, flags = struct.unpack('<IIII', footer[8:24])
        print(f"APE_footer_offset={end - 32}, version={version}, tag_bytes={tag_size}, item_count={count}, flags=0x{flags:08x}", file=out)
        print("footer_hex=" + footer.hex(), file=out)
        if tag_size < 32 or tag_size > end or flags & 0x20000000:
            raise ValueError("invalid footer size or header flag; refusing to parse items")
        start = end - tag_size
        print(f"item_region=[{start}, {end - 32})", file=out)
        if start >= 32 and read_at(stream, start - 32, 8) == b'APETAGEX':
            print("optional_header_hex=" + read_at(stream, start - 32, 32).hex(), file=out)
        tag_items(stream, start, end - 32, count, out)


def inspect_cue(path, out):
    print("\n=== CUE FILE ===", file=out)
    print("name=" + quoted(path.name), file=out)
    size = path.stat().st_size
    print(f"file_bytes={size}", file=out)
    if size > MAX_CUE:
        print(f"OMITTED: exceeds {MAX_CUE} byte CUE limit", file=out)
        return
    with path.open('rb') as stream:
        data = read_at(stream, 0, size)
    describe_text(data, "CUE", out, preview=12000)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ape', type=Path, help='APE file to inspect (read-only)')
    parser.add_argument('--cue', type=Path, action='append', help='CUE file; repeat for multiple files. Default: all .cue files in the APE directory.')
    parser.add_argument('-o', '--output', type=Path, help='Write UTF-8 report to a NEW file; never overwrite an existing file.')
    args = parser.parse_args()
    if not args.ape.is_file():
        parser.error('APE file does not exist or is not a regular file')
    report = io.StringIO()
    print('Voon APE/CUE diagnostic v1', file=report)
    print('Read-only metadata inspection. No audio decoding or library writes.', file=report)
    print('Raw text bytes are included for reproducibility; binary APE items are omitted.', file=report)
    print('Encoding candidates are evidence for review, not automatic encoding decisions.', file=report)
    failed = False
    try:
        inspect_ape(args.ape, report)
    except (OSError, ValueError) as error:
        failed = True
        print('APE inspection error: ' + quoted(error), file=report)
    try:
        cues = args.cue if args.cue is not None else sorted(p for p in args.ape.parent.iterdir() if p.is_file() and p.suffix.lower() == '.cue')
    except OSError as error:
        cues = []
        failed = True
        print('CUE discovery error: ' + quoted(error), file=report)
    if not cues:
        print('\nNo CUE files found; specify --cue if stored elsewhere.', file=report)
    for cue in cues:
        try:
            inspect_cue(cue, report)
        except (OSError, ValueError) as error:
            failed = True
            print('CUE inspection error: ' + quoted(error), file=report)
    print('\n=== END OF REPORT ===', file=report)
    if args.output:
        try:
            with args.output.open('x', encoding='utf-8') as target:
                target.write(report.getvalue())
        except OSError as error:
            parser.error(f'cannot create report: {error}; choose a new output filename')
        print(f'Report saved: {args.output}')
    else:
        # ASCII escapes remain recoverable if the terminal cannot print a tag character.
        sys.stdout.reconfigure(errors='backslashreplace')
        sys.stdout.write(report.getvalue())
    return 1 if failed else 0


if __name__ == '__main__':
    raise SystemExit(main())
