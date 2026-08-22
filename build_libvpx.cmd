@echo off
rem ============================================================
rem  build_libvpx.cmd  -  one-shot libvpx static lib build
rem  v1.14.1, pure-C VP9 decoder (640x360 max), MSVC x86_64-win64-vs17
rem  flow: configure -> make (gen vpx.vcxproj, AS=nasm) -> patch include
rem        root -> msbuild Release/x64
rem  output: vendor_libvpx\x64\Release\vpxmd.lib (~5.8MB)
rem  prereq: VS2022 (MSVC), nasm (winget install NASM.NASM; only needed
rem  for float_control_word.asm), WSL (bash/wslpath/make/perl)
rem ============================================================
setlocal
set VCVARS=C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat
set NASM=C:\Program Files\nasm
if not exist "%VCVARS%" (
  echo [FAIL] VS2022 vcvars64.bat not found
  exit /b 1
)
if not exist "%NASM%\nasm.exe" (
  echo [FAIL] nasm not found - run: winget install NASM.NASM
  exit /b 1
)
call "%VCVARS%" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
set PATH=%NASM%;%PATH%
echo [OK] vcvars + nasm ready

rem repo -> WSL path
for /f "delims=" %%i in ('bash -c "wslpath -u '%~dp0vendor_libvpx'"') do set VENDOR_W=%%i
if "%VENDOR_W%"=="" (
  echo [FAIL] wslpath failed - WSL required
  exit /b 1
)

echo [STEP 1] configure
rem NOTE: all x86 SIMD disabled (pure C). The v1.14.1 asm decode path computes
rem wrong pixels randomly at high-motion edges (same video decoded twice:
rem 146~232 frames differ, up to 2461 px/frame, whole-block hue errors vs
rem ffmpeg reference). Pure C + 1-thread decode is frame-deterministic.
bash -c "cd '%VENDOR_W%' && ./configure --target=x86_64-win64-vs17 --enable-external-build --disable-examples --disable-tools --disable-docs --disable-unit-tests --disable-vp8-decoder --disable-vp8-encoder --disable-vp9-encoder --size-limit=640x360 --disable-install-bins --disable-install-srcs --disable-mmx --disable-sse --disable-sse2 --disable-sse3 --disable-ssse3 --disable-sse4_1 --disable-avx --disable-avx2 --disable-avx512 > configure.log 2>&1; ec=$?; tail -6 configure.log; exit $ec"
if errorlevel 1 (
  echo [FAIL] configure failed
  exit /b 1
)
echo [OK] configure done

echo [STEP 2] generate vpx.vcxproj (make target=libs)
rem force-regen rtcd/config artifacts: rtcd rules only depend on defs.pl and
rem will not notice configure changes (stale SIMD refs would remain)
bash -c "cd '%VENDOR_W%' && rm -f vp9_rtcd.h vpx_dsp_rtcd.h vpx_scale_rtcd.h vpx_config.asm vpx.vcxproj vpxrc.vcxproj && make target=libs CFLAGS='-DNDEBUG' AS=nasm > gen_vcxproj.log 2>&1; ec=$?; tail -4 gen_vcxproj.log; exit $ec"
if errorlevel 1 (
  echo [FAIL] vcxproj generation failed
  exit /b 1
)

echo [STEP 3] patch include root (ProjectDir)
rem %% inside a batch lives as a literal percent
powershell -NoProfile -Command "$f='%~dp0vendor_libvpx\vpx.vcxproj'; $t=[System.IO.File]::ReadAllText($f); $n=$t.Replace('<AdditionalIncludeDirectories>;%%','<AdditionalIncludeDirectories>$(ProjectDir);%%'); [System.IO.File]::WriteAllText($f,$n); if(-not $n.Contains('$(ProjectDir);')){throw 'patch failed'}"
if errorlevel 1 (
  echo [FAIL] vcxproj patch failed
  exit /b 1
)
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
