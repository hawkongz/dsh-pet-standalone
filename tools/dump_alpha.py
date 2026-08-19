#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""精确解析 BlockGroup 的 alpha 段结构。"""
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
    return eid, sz, p2, w

def children(data, p, ln):
    end = p + ln; pos = p
    while pos < end:
        r = elem(data, pos)
        if r is None: break
        eid, sz, p2, w = r
        yield eid, sz, p2, w, pos
        pos = p2 + sz

path = sys.argv[1] if len(sys.argv) > 1 else r"C:\allsoftware\devs\ds-pet\assets\videos\待机呼吸休闲.webm"
data = open(path, 'rb').read()
# 定位 Segment -> Cluster -> BlockGroup #0
for eid, sz, p2, w, pos in children(data, 0, len(data)):
    if eid == 0x18538067: sp, sl = p2, sz; break
for eid, sz, p2, w, pos in children(data, sp, sl):
    if eid == 0x1F43B675: cp, cl = p2, sz; break
for eid, sz, p2, w, pos in children(data, cp, cl):
    if eid == 0xA0:
        bgp, bgl = p2, sz; break

print(f"BlockGroup #0 payload @ {bgp}, {bgl} bytes, ends {bgp+bgl}")
# 遍历 BlockGroup 的所有元素，打印精确偏移
n = 0
for eid, sz, p2, w, pos in children(data, bgp, bgl):
    n += 1
    print(f"  [{n}] id=0x{eid:x} size={sz} @ {pos} (id_bytes={w}) end={p2+sz} head={data[p2:p2+min(sz,28)].hex()}")
print(f"BlockGroup 内元素总数: {n}")

# 分析 Block 之后的 alpha 段：从第一个元素后到 BlockGroup 尾
# Block 是第一个元素
first_eid, first_sz, first_p2, first_w, first_pos = None, None, None, None, None
# 找 Block
for eid, sz, p2, w, pos in children(data, bgp, bgl):
    if eid == 0xA1:
        block_end = p2 + sz
        print(f"\nBlock payload @ {p2} size={sz}, block ends @ {block_end}")
        rest = data[block_end:bgp+bgl]
        print(f"Block 后剩余: {len(rest)} bytes")
        print(f"rest head: {rest[:60].hex()}")
        break
