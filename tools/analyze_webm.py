#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""分析 webm EBML：Tracks 元数据 + Cluster 帧统计 + BlockAdditional(alpha) 形态。"""
import sys

def read_vint(data, pos):
    """EBML vint: (值, 新pos, 宽度)。"""
    b = data[pos]
    width = 1
    mask = 0x80
    while not (b & mask):
        mask >>= 1
        width += 1
    value = b & (mask - 1)
    for i in range(1, width):
        value = (value << 8) | data[pos + i]
    return value, pos + width, width

def elem(data, pos):
    """(id, size, payload, plen) 或 None。"""
    if pos >= len(data):
        return None
    b = data[pos]
    w = 1
    m = 0x80
    while not (b & m):
        m >>= 1
        w += 1
    eid = int.from_bytes(data[pos:pos+w], 'big')
    size, p2, sw = read_vint(data, pos + w)
    return eid, size, p2, size

def children(data, pstart, plen):
    """迭代子元素 (eid, payload, plen)。"""
    pos = pstart
    end = pstart + plen
    while pos < end:
        r = elem(data, pos)
        if r is None:
            break
        eid, size, p2, slen = r
        yield eid, p2, slen
        pos = p2 + slen

def main():
    path = sys.argv[1] if len(sys.argv) > 1 else r"C:\allsoftware\devs\ds-pet\assets\videos\待机呼吸休闲.webm"
    data = open(path, 'rb').read()
    print(f"== {path} ({len(data)} bytes) ==")

    # 定位 Segment
    seg = None
    for eid, p, ln in children(data, 0, len(data)):
        if eid == 0x18538067:
            seg = (p, ln)
            break
    if not seg:
        print("no segment"); return
    sp, slen = seg

    # Tracks
    track_alpha = 0
    track_codec = None
    track_w = track_h = 0
    cluster_frames = 0
    cluster_bytes = 0
    n_blockgroup = 0
    n_blockadd = 0
    blockadd_sizes = []
    first_block = None
    first_blockadd_head = None

    for eid, p, ln in children(data, sp, slen):
        if eid == 0x1549A966:  # Info
            for e2, p2, ln2 in children(data, p, ln):
                if e2 == 0x2AD7B1:  # TimecodeScale
                    print(f"Info.TimecodeScale = {int.from_bytes(data[p2:p2+ln2],'big')}")
        elif eid == 0x1654AE6B:  # Tracks
            for e2, p2, ln2 in children(data, p, ln):
                if e2 == 0xAE:  # TrackEntry
                    for e3, p3, ln3 in children(data, p2, ln2):
                        if e3 == 0x86:  # CodecID
                            track_codec = data[p3:p3+ln3].decode('utf-8', 'replace')
                        elif e3 == 0x53C0:  # AlphaMode
                            track_alpha = int.from_bytes(data[p3:p3+ln3], 'big')
                        elif e3 == 0xE0:  # Video
                            for e4, p4, ln4 in children(data, p3, ln3):
                                if e4 == 0xB0: track_w = int.from_bytes(data[p4:p4+ln4],'big')
                                if e4 == 0xBA: track_h = int.from_bytes(data[p4:p4+ln4],'big')
        elif eid == 0x1F43B675:  # Cluster
            for e2, p2, ln2 in children(data, p, ln):
                if e2 == 0xA3:  # SimpleBlock
                    cluster_frames += 1
                    if first_block is None:
                        # SimpleBlock payload: track vint + int16 timecode + flags + data
                        tv, tp, _ = read_vint(data, p2)
                        tc = int.from_bytes(data[tp:tp+2], 'big', signed=True)
                        flags = data[tp+2]
                        lacing = flags & 0x06
                        keyframe = (flags >> 7) & 1
                        first_block = (tv, tc, keyframe, lacing)
                elif e2 == 0xA0:  # BlockGroup
                    n_blockgroup += 1
                    for e3, p3, ln3 in children(data, p2, ln2):
                        if e3 == 0xE4:  # BlockAdditional
                            n_blockadd += 1
                            blockadd_sizes.append(ln3)
                            if first_blockadd_head is None and ln3 > 0:
                                first_blockadd_head = data[p3:p3+min(ln3, 24)].hex()
                            # 深入 BlockAdditional 看 BlockAddID
                            for e4, p4, ln4 in children(data, p3, ln3):
                                if e4 == 0xEE:
                                    pass
    print(f"Track: codec={track_codec} w={track_w} h={track_h} alpha_mode={track_alpha}")
    print(f"SimpleBlocks={cluster_frames}, BlockGroups={n_blockgroup}, BlockAdditional={n_blockadd}")
    print(f"first SimpleBlock: {first_block}")
    if blockadd_sizes:
        print(f"BlockAdditional sizes: min={min(blockadd_sizes)} max={max(blockadd_sizes)} avg={sum(blockadd_sizes)//len(blockadd_sizes)} n={len(blockadd_sizes)}")
        print(f"first BlockAdditional head: {first_blockadd_head}")

if __name__ == "__main__":
    main()
