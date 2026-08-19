@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
cd /d C:\allsoftware\devs\ds-pet\vendor_libvpx
echo [START] nmake
nmake > build_nmake.log 2>&1
if errorlevel 1 (
  echo [FAIL] nmake failed
  tail -40 build_nmake.log
  exit /b 1
)
echo [OK] nmake done
echo "=== LIB OUTPUT ==="
dir *.lib 2>nul
