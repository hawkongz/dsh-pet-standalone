@echo off
rem 一键构建 libvpx 静态库（v1.14.1，仅 VP9 解码器，640x360 上限，MSVC x86_64-win64-vs17）
rem 流程：configure -> make 生成 vpx.vcxproj（AS=nasm）-> 修正 include 根 -> msbuild Release/x64
rem 产物：vendor_libvpx\x64\Release\vpxmd.lib（~7.6MB，链接后 dead-code 消除）
rem 依赖：Visual Studio 2022（MSVC）、nasm（winget install NASM.NASM）、WSL（bash/wslpath/make/perl）
setlocal
set VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat
set NASM=C:\Program Files\nasm
if not exist "%VCVARS%" (
  echo [FAIL] 未找到 VS2022 vcvars64.bat
  exit /b 1
)
if not exist "%NASM%\nasm.exe" (
  echo [FAIL] 未找到 nasm（请先 winget install NASM.NASM）
  exit /b 1
)
call "%VCVARS%" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
set PATH=%NASM%;%PATH%
echo [OK] vcvars + nasm ready

rem 仓库 -> WSL 路径
for /f "delims=" %%i in ('bash -c "wslpath -u '%~dp0vendor_libvpx'"') do set VENDOR_W=%%i
if "%VENDOR_W%"=="" (
  echo [FAIL] wslpath 失败（需要 WSL）
  exit /b 1
)

echo [STEP 1] configure
bash -c "cd '%VENDOR_W%' && ./configure --target=x86_64-win64-vs17 --enable-external-build --disable-examples --disable-tools --disable-docs --disable-unit-tests --disable-vp8-decoder --disable-vp8-encoder --disable-vp9-encoder --size-limit=640x360 --disable-install-bins --disable-install-srcs --disable-avx512 > configure.log 2>&1; ec=$?; tail -6 configure.log; exit $ec"
if errorlevel 1 (
  echo [FAIL] configure failed
  exit /b 1
)
echo [OK] configure done

echo [STEP 2] generate vpx.vcxproj (make target=libs)
del /q "%~dp0vendor_libvpx\vpx.vcxproj" "%~dp0vendor_libvpx\vpxrc.vcxproj" 2>nul
bash -c "cd '%VENDOR_W%' && make target=libs CFLAGS='-DNDEBUG' AS=nasm > gen_vcxproj.log 2>&1; ec=$?; tail -4 gen_vcxproj.log; exit $ec"
if errorlevel 1 (
  echo [FAIL] vcxproj generation failed
  exit /b 1
)

echo [STEP 3] patch include root (ProjectDir)
powershell -NoProfile -Command "$f='%~dp0vendor_libvpx\vpx.vcxproj'; $t=[System.IO.File]::ReadAllText($f); $n=$t.Replace('<AdditionalIncludeDirectories>;%','<AdditionalIncludeDirectories>$(ProjectDir);%'); [System.IO.File]::WriteAllText($f,$n)"
echo [OK] patched

echo [STEP 4] msbuild vpx.vcxproj Release/x64
msbuild "%~dp0vendor_libvpx\vpx.vcxproj" /p:Configuration=Release /p:Platform=x64 /m /v:minimal > "%~dp0vendor_libvpx\msbuild_vpx.log" 2>&1
if errorlevel 1 (
  echo [FAIL] msbuild failed, tail of log:
  powershell -NoProfile -Command "Get-Content '%~dp0vendor_libvpx\msbuild_vpx.log' -Tail 30"
  exit /b 1
)
echo [OK] msbuild done
echo "=== LIB ==="
dir /s /b "%~dp0vendor_libvpx\x64\Release\*.lib" 2>nul
endlocal
