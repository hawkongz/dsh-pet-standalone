#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""严格解析 webm：dump Segment 级元素树，特别关注 Block/BlockAdditional/alpha。"""
import sys

def vint(data, pos):
    b = data[pos]
    w = 1; m = 0x80
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

IDS = {
    0x1A45DFA3:'EBML', 0x18538067:'Segment', 0x1549A966:'Info', 0x2AD7B1:'TimecodeScale',
    0x1654AE6B:'Tracks', 0xAE:'TrackEntry', 0xD7:'TrackNumber', 0x83:'TrackType',
    0x86:'CodecID', 0xE0:'Video', 0xB0:'PixelWidth', 0xBA:'PixelHeight',
    0x53C0:'AlphaMode', 0x1F43B675:'Cluster', 0xE7:'Timecode',
    0xA3:'SimpleBlock', 0xA0:'BlockGroup', 0xA1:'Block', 0xE4:'BlockAdditional',
    0xEE:'BlockAddID', 0x9D:'BlockDuration', 0x75A1:'DiscardPadding',
}

def walk(data, pos, end, depth, maxd):
    while pos < end:
        eid, sz, p2 = elem(data, pos)
        if p2 + sz > end + 1000000:
            print(f"{'  '*depth}[corrupt] {hex(eid)} sz={sz} @ {pos}")
            break
        name = IDS.get(eid, f'0x{eid:x}')
        print(f"{'  '*depth}{name} id=0x{eid:x} size={sz} @ {pos}")
        payload = p2
        plen = sz
        if eid in (0xA3, 0xA1):
            # Block/SimpleBlock payload
            tv, tp, _ = vint(data, payload)
            tc = int.from_bytes(data[tp:tp+2], 'big', signed=True)
            flags = data[tp+2]
            dlen = plen - (tp+3-payload)
            print(f"{'  '*depth}  block track={tv} tc={tc} flags=0x{flags:02x} datalen={dlen}")
            print(f"{'  '*depth}  datahead={data[tp+3:tp+19].hex()}")
        if eid == 0x1A45DFA3 or eid == 0x18538067 or eid == 0x1654AE6B or eid == 0xAE or eid == 0x1F43B675 or eid == 0xA0:
            if depth < maxd:
                walk(data, p2, p2+sz, depth+1, maxd)
        pos = p2 + sz

path = sys.argv[1] if len(sys.argv) > 1 else r"C:\allsoftware\devs\ds-pet\assets\videos\待机呼吸休闲.webm"
data = open(path, 'rb').read()
print(f"== {path} ({len(data)}B) ==")
walk(data, 0, len(data), 0, 2)
