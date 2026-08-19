@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
set PATH=C:\Users\13867\AppData\Local\bin\NASM;%PATH%
cd /d C:\allsoftware\devs\ds-pet\vendor_libvpx
echo [START] msbuild vpx.vcxproj Release/x64
"C:\Program Files\Microsoft Visual Studio\18\Community\MSBuild\Current\Bin\MSBuild.exe" vpx.vcxproj /p:Configuration=Release /p:Platform=x64 /m /v:minimal > msbuild_vpx.log 2>&1
if errorlevel 1 (
  echo [FAIL] msbuild failed
  tail -25 msbuild_vpx.log
  exit /b 1
)
echo [OK] msbuild done
echo "=== LIB ==="
dir /s /b *.lib 2>nul
