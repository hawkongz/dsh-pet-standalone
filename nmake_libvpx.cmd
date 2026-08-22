@echo off
rem 已由 build_libvpx.cmd 取代（nmake 无法解析 libvpx 的 GNU make 语法 config.mk）。
echo [SKIP] nmake 路线不适用于 libvpx（config.mk 为 GNU make 语法），
echo        请改用 build_libvpx.cmd（configure + vcxproj + msbuild 一键流程）。
exit /b 0
