#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""dump BlockGroup #0 的完整二进制，解析 Block 后的 alpha 数据结构。"""
import sys

def vint(data, pos):
    b = data[pos]; w = 1; m = 0x80
    while not (b & m): m >>= 1; w += 1
    v = b & (m - 1)
    for i in range(1, w): v = (v << 8) | data[pos+i]
    return v, pos+w, w

def elem(data, pos):
    b = data[pos]; w = 1; m = 0x80
    while not (b & m): m >>= 1; w += 1
    eid = int.from_bytes(data[pos:pos+w], 'big')
    sz, p2, _ = vint(data, pos+w)
    return eid, sz, p2

def children(data, p, ln):
    end = p + ln; pos = p
    while pos < end:
        r = elem(data, pos)
        if r is None: break
        eid, sz, p2 = r
        yield eid, sz, p2
        pos = p2 + sz

path = sys.argv[1] if len(sys.argv) > 1 else r"C:\allsoftware\devs\ds-pet\assets\videos\待机呼吸休闲.webm"
data = open(path, 'rb').read()
for eid, sz, p2 in children(data, 0, len(data)):
    if eid == 0x18538067: sp, sl = p2, sz; break
for eid, sz, p2 in children(data, sp, sl):
    if eid == 0x1F43B675: cp, cl = p2, sz; break
# 第一个 BlockGroup
for eid, sz, p2 in children(data, cp, cl):
    if eid == 0xA0:
        bg = (p2, sz); break
bp, bl = bg
print(f"BlockGroup @ {bp}, size={bl}")
print("payload head 80 bytes:", data[bp:bp+80].hex())
# 遍历 BlockGroup 子元素
print("\n=== BlockGroup 子元素 ===")
for eid, sz, p2 in children(data, bp, bl):
    name = {0xA1:'Block', 0xE4:'BlockAdditional', 0xEE:'BlockAddID', 0x81:'BlockAdditionalData',
            0x9D:'BlockDuration', 0x75A1:'DiscardPadding', 0xFB:'BlockAddIdType',
            0x7B7A:'BlockAddIDExtraData', 0x9B:'BlockPriority'}.get(eid, f'0x{eid:x}')
    print(f"  {name} id=0x{eid:x} size={sz} @ {p2}")
    print(f"    head: {data[p2:p2+min(sz,40)].hex()}")
