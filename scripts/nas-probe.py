#!/usr/bin/env python3
"""Read-only filename diagnostics; run on the NAS itself, not an SMB client."""

import argparse
import json
import os
import stat
import sys
import unicodedata as u

parser = argparse.ArgumentParser(description="在 NAS 本机只读检查 13 个文件的文件名和文件头。")
parser.add_argument("--music-root", default=os.environ.get("MUSIC_ROOT", ""),
                    help="NAS 本机音乐目录的绝对路径，也可通过 MUSIC_ROOT 环境变量指定")
root = parser.parse_args().music_root
if not root or not os.path.isabs(root) or not os.path.isdir(root):
    parser.error("请通过 --music-root 指定存在的 NAS 本机音乐目录绝对路径。")

def emit(value):
    print(json.dumps(value, ensure_ascii=True), flush=True)

def probe(fd, name):
    handle = None
    try:
        flags = os.O_RDONLY | os.O_NONBLOCK | getattr(os, "O_NOFOLLOW", 0)
        handle = os.open(name, flags, dir_fd=fd)
        info = os.fstat(handle)
        result = {"open": "ok", "inode": info.st_ino, "device": info.st_dev,
                  "size": info.st_size, "mode": oct(stat.S_IMODE(info.st_mode))}
        if stat.S_ISREG(info.st_mode):
            result["header_hex"] = os.read(handle, 4).hex()
        else:
            result["header_skipped"] = "not a regular file"
        return result
    except OSError as error:
        return {"open_or_read": "failed", "errno": error.errno, "error": error.strerror}
    finally:
        if handle is not None:
            os.close(handle)

groups = [
    ("Sarah Alainn《Timeless ～》24-96",
     ["04 比べられない.flac", "22 我が故郷 (Orchestral ver.).flac"], False),
    ("莫扎特：小提琴与键盘奏鸣曲集",
     ["%02d.Sonate für Klavier und Violine " % n for n in range(1, 12)], True),
]
emit({"report": "voon-nas-readonly-v1", "root": root,
      "python": sys.version.split()[0], "platform": sys.platform, "expected": 13})
selected = 0
raw_readable = 0
for folder, targets, prefix_match in groups:
    path = os.fsencode(os.path.join(root, folder))
    fd = None
    try:
        fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
        with os.scandir(path) as entries:
            names = [entry.name for entry in entries]
        for target in targets:
            matches = []
            for name in names:
                normalized = u.normalize("NFC", os.fsdecode(name))
                matched = normalized.startswith(target) and normalized.lower().endswith(".flac") if prefix_match else normalized == target
                if matched:
                    matches.append(name)
            if len(matches) != 1:
                emit({"folder": folder, "target": target, "match_count": len(matches),
                      "candidate_bytes_hex": [name.hex() for name in matches]})
                continue
            raw = matches[0]
            selected += 1
            result = {"folder": folder, "name": os.fsdecode(raw),
                      "raw_name_hex": raw.hex(), "raw": probe(fd, raw)}
            raw_readable += "header_hex" in result["raw"]
            try:
                decoded = raw.decode("utf-8", "strict")
                result["valid_utf8"] = True
                for form in ("NFC", "NFD"):
                    variant = u.normalize(form, decoded).encode("utf-8")
                    result[form] = {
                        "same_bytes_as_raw": variant == raw,
                        "name_hex": variant.hex(),
                        "result": probe(fd, variant),
                    }
            except UnicodeDecodeError:
                result["valid_utf8"] = False
            emit(result)
    except OSError as error:
        emit({"folder": folder, "directory_error": error.strerror, "errno": error.errno})
    finally:
        if fd is not None:
            os.close(fd)
emit({"summary": {"expected": 13, "selected": selected,
                  "raw_header_readable": raw_readable}})
