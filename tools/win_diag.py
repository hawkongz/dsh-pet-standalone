import ctypes
from ctypes import wintypes
import sys

user32 = ctypes.windll.user32
EnumWindows = user32.EnumWindows
GetClassNameW = user32.GetClassNameW
GetWindowRect = user32.GetWindowRect
IsWindowVisible = user32.IsWindowVisible
GetWindowTextW = user32.GetWindowTextW

WNDENUMPROC = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)

results = []

def cb(hwnd, lparam):
    cls = ctypes.create_unicode_buffer(256)
    GetClassNameW(hwnd, cls, 256)
    if 'dsh_pet' in cls.value.lower() or 'pet' in cls.value.lower():
        r = wintypes.RECT()
        GetWindowRect(hwnd, ctypes.byref(r))
        vis = IsWindowVisible(hwnd)
        txt = ctypes.create_unicode_buffer(256)
        GetWindowTextW(hwnd, txt, 256)
        results.append((hwnd, cls.value, (r.left, r.top, r.right, r.bottom), vis, txt.value))
    return True

EnumWindows(WNDENUMPROC(cb), 0)
print(f"找到 {len(results)} 个相关窗口:")
for hwnd, cls, rect, vis, txt in results:
    w = rect[2] - rect[0]
    h = rect[3] - rect[1]
    print(f"  hwnd={hwnd} class='{cls}' rect={rect} size={w}x{h} visible={vis} title='{txt}'")

if not results:
    print("未找到 dsh-pet 窗口！检查进程是否运行")
    # 检查进程
    import subprocess
    out = subprocess.run(['tasklist', '/FI', 'IMAGENAME eq dsh-pet.exe'], capture_output=True, text=True).stdout
    print(out[-300:])
