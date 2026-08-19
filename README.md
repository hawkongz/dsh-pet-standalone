# dsh-pet-standalone（Rust / Win32 原生版）

把 [dsh-pet](https://github.com/PC2005-cloud/dsh-pet) 插件里的桌宠重构为**单 exe 桌面宠物**，
用 **Rust + 纯 Win32 原生 API** 实现，**直接解码上游 51 段 640×360 透明 webm**（VP9 + alpha）。

> **声明与致谢**：素材动画、动画链行为模型、交互设计均来自 [dsh-pet](https://github.com/PC2005-cloud/dsh-pet) 与
> [dsh-pet-indesktop](https://github.com/MerZlin/dsh-pet-indesktop)（原 Python 版），特此声明并致谢。

## 体积对比

| 版本 | 技术栈 | exe 体积 | 运行时依赖 | 启动 |
|---|---|---|---|---|
| 原版 | Python + PySide6 + imageio-ffmpeg | ~110MB（onefile 自解压壳） | 解压素材，5~15s | 慢 |
| **本版** | **Rust + libvpx 静态链接 + Win32 API** | **~36MB 单文件** | **零**（素材已内嵌） | 秒开 |

程序本体仅约 **1MB**，其余 35MB 为内嵌的 51 段高清 webm 动画（用户可接受素材体积）。
GUI 全部用 Win32 原生 API（`CreateWindowExW` / `UpdateLayeredWindow` / `Shell_NotifyIconW` /
`TrackPopupMenu` / 注册表），无任何 GUI 框架。

## 特性

- **多桌宠**：右键菜单「生成新桌宠」可创建多只，每只独立动画链/位置/朝向/大小/置顶；「删除此桌宠」移除当前，配置 `pets` 数组自动持久化，启动时全部恢复
- **动画链**：30% 待机 / 10% 转向 / 40% 随机动作 / 20% 移动，永不停止
- **透明窗口**：`UpdateLayeredWindow` 逐像素 alpha 合成，半透明边缘保留
- **鼠标穿透**：`WM_NCHITTEST` 按像素 alpha（<128 穿透），等效原版命中层
- **点击回应**：待机时点击随机播 3 种回应动画
- **拖拽**：`SetCapture` 捕获鼠标 + 全局坐标跟手，快速拖拽不丢事件
- **右键菜单**：播放任意动画 / 回到右下角 / 置顶 / 不移动 / 开机自启 / 4 档大小 / 生成新桌宠 / 删除此桌宠
- **系统托盘**：显示/隐藏全部、生成新桌宠、开机自启、退出
- **开机自启**：HKCU\...\Run 注册表键（无需管理员）
- **VP9 alpha 解码**：静态链接 libvpx，主色 + BlockAdditional alpha 双路解码合成 RGBA

## 快速开始

```sh
# 1. 准备素材：从上游 dsh-pet 仓库获取 51 个 webm，放 assets/videos/
#    （GitHub: PC2005-cloud/dsh-pet → dsh-pet/assets/thumb/*.webm）

# 2. 首次构建需编译 libvpx 静态库（见下），之后：
cargo build --release

# 3. 运行
target/release/dsh-pet.exe
```

## 构建 libvpx 静态库（首次）

需要：Visual Studio 2026（MSVC）、nasm、MSYS2 make（`tools_msys/`）。

```bat
git clone --depth 1 --branch v1.14.1 https://github.com/webmproject/libvpx vendor_libvpx
build_libvpx.cmd   rem configure（禁用 vp8/encoder，仅 VP9 解码器）
```

产物：`vendor_libvpx/x64/Release/vpxmd.lib`（~7.6MB，链接后 dead-code 消除）。

## 目录结构

```
├── src/
│   ├── main.rs         # 入口 + 托盘图标生成
│   ├── win32.rs        # 纯 Win32 窗口（透明 + 穿透 + 渲染 + 消息循环）
│   ├── app.rs          # 应用逻辑（状态机 + 交互 + 菜单）
│   ├── state.rs        # 动画目录 + 动画链概率
│   ├── clip.rs         # 解码管线（VP9 主色 + alpha → BGRA 合成）
│   ├── webm.rs         # WebM/EBML 解析（BlockAdditions alpha）
│   ├── vpx.rs          # libvpx FFI 绑定（静态链接 vpxmd.lib）
│   ├── config.rs       # 配置持久化（%APPDATA%/dsh-pet-standalone）
│   ├── autostart.rs    # 注册表开机自启
│   ├── tray.rs         # 系统托盘（Shell_NotifyIconW）
│   └── monitor.rs      # 屏幕工作区
├── build.rs            # 素材打包（51 webm → assets.pak + 索引）
├── tools/              # 验证/诊断脚本
├── tools_msys/         # MSYS2 make（libvpx 构建）
├── vendor_libvpx/      # libvpx 源码 + vpxmd.lib
└── build_libvpx.cmd    # libvpx 构建脚本
```

## 素材解码管线

```
webm 文件 → EBML 解析（Segment/Tracks/Cluster）
  ├─ Block (0xA1)        → VP9 主色帧 → libvpx 解码 → YUV420
  └─ BlockAdditions      → alpha VP9 灰度帧 → libvpx 解码 → alpha 平面
      (0x75A1 > BlockMore 0xA6 > BlockAdditional 0xA5)
                        → 合成 BGRA premultiplied（BT.601 limited）
                        → 缩放 + 镜像 → UpdateLayeredWindow
```

## 验证

- `cargo run --bin decode_check`：解析 51 个动画（12291 帧）、alpha 分布校验、解码性能（<40ms/帧）
- `tools/win_diag.py`：窗口诊断（位置/尺寸/可见性）
