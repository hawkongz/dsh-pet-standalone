#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""dump 前几个 BlockGroup 的完整字节结构，找 alpha 数据位置。"""
import sys

def read_vint(data, pos):
    b = data[pos]
    width = 1; mask = 0x80
    while not (b & mask):
        mask >>= 1; width += 1
    value = b & (mask - 1)
    for i in range(1, width):
        value = (value << 8) | data[pos + i]
    return value, pos + width, width

def elem(data, pos):
    if pos >= len(data): return None
    b = data[pos]; w = 1; m = 0x80
    while not (b & m): m >>= 1; w += 1
    eid = int.from_bytes(data[pos:pos+w], 'big')
    size, p2, sw = read_vint(data, pos + w)
    return eid, size, p2, size

def children(data, p, ln):
    end = p + ln; pos = p
    while pos < end:
        r = elem(data, pos)
        if r is None: break
        eid, size, p2, slen = r
        yield eid, p2, slen
        pos = p2 + slen

path = sys.argv[1] if len(sys.argv) > 1 else r"C:\allsoftware\devs\ds-pet\assets\videos\待机呼吸休闲.webm"
data = open(path, 'rb').read()
# Segment
for eid, p, ln in children(data, 0, len(data)):
    if eid == 0x18538067:
        seg = (p, ln); break
sp, slen = seg
# 找第一个 Cluster
for eid, p, ln in children(data, sp, slen):
    if eid == 0x1F43B675:
        cluster = (p, ln); break
cp, clen = cluster
print(f"Cluster at {cp}, {clen} bytes")
# Cluster 前几个元素
count = 0
for eid, p2, ln2 in children(data, cp, clen):
    if eid == 0xA0:  # BlockGroup
        print(f"\n--- BlockGroup #{count} ({ln2} bytes) @ {p2} ---")
        print("  raw head:", data[p2:p2+min(ln2, 30)].hex())
        for e3, p3, ln3 in children(data, p2, ln2):
            name = {0xA1:'Block', 0xE4:'BlockAdditional', 0x9D:'BlockDuration',
                    0xEE:'BlockAddID', 0x81:'BlockAdditionalData', 0x75A1:'DiscardPadding'}.get(e3, hex(e3))
            if e3 == 0xA1:
                # Block payload: tracknum vint + int16 tc + flags
                tv, tp, _ = read_vint(data, p3)
                tc = int.from_bytes(data[tp:tp+2], 'big', signed=True)
                flags = data[tp+2]
                bdata = data[tp+3:p3+ln3]
                print(f"  Block[{name}] size={ln3} track={tv} tc={tc} flags=0x{flags:02x} datalen={len(bdata)}")
                print(f"    data head: {bdata[:16].hex()}")
            else:
                print(f"  {name} size={ln3}: head={data[p3:p3+min(ln3,12)].hex()}")
        count += 1
        if count >= 5: break
