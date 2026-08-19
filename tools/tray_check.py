import ctypes
from ctypes import wintypes

user32 = ctypes.windll.user32
shell32 = ctypes.windll.shell32

FindWindowW = user32.FindWindowW
FindWindowExW = user32.FindWindowExW
SendMessageW = user32.SendMessageW

# 托盘工具栏按钮枚举
TB_BUTTONCOUNT = 0x0418
TB_GETBUTTONTEXTW = 0x043F
TB_GETBUTTON = 0x0417

class TBBUTTON(ctypes.Structure):
    _fields_ = [
        ("iBitmap", ctypes.c_int),
        ("idCommand", ctypes.c_int),
        ("fsState", ctypes.c_ubyte),
        ("fsStyle", ctypes.c_ubyte),
        ("bReserved", ctypes.c_ubyte),
        ("dwData", ctypes.c_ulong),
        ("iString", ctypes.c_void_p),
    ]

def find_tray_buttons():
    """枚举系统托盘所有图标及其 tooltip。"""
    tray = FindWindowW("Shell_TrayWnd", None)
    if not tray:
        print("找不到任务栏")
        return
    notify = FindWindowExW(tray, 0, "TrayNotifyWnd", None)
    if not notify:
        print("找不到托盘区域")
        return
    # 可能有多个 toolbar
    def walk_toolbars(parent):
        tb = FindWindowExW(parent, 0, "ToolbarWindow32", None)
        found = []
        while tb:
            found.append(tb)
            tb = FindWindowExW(parent, tb, "ToolbarWindow32", None)
        return found
    bars = walk_toolbars(notify)
    if not bars:
        bars = walk_toolbars(tray)
    results = []
    for tb in bars:
        count = SendMessageW(tb, TB_BUTTONCOUNT, 0, 0)
        for i in range(count):
            btn = TBBUTTON()
            if not SendMessageW(tb, TB_GETBUTTON, i, ctypes.byref(btn)):
                continue
            # 读 tooltip 文本（iString 指向内部缓冲）
            buf = ctypes.create_unicode_buffer(256)
            n = SendMessageW(tb, TB_GETBUTTONTEXTW, btn.idCommand, buf)
            if n > 0:
                results.append(buf.value)
    return results

tips = find_tray_buttons()
print(f"托盘图标 {len(tips) if tips else 0} 个")
if tips:
    for t in tips:
        print(" -", t)
    if any('dsh-pet' in t or 'dsh' in t.lower() for t in tips):
        print(">>> dsh-pet 托盘图标存在!")
    else:
        print(">>> 未找到 dsh-pet 托盘图标")
