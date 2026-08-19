@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
echo [OK] vcvars loaded
cd /d C:\allsoftware\devs\ds-pet\vendor_libvpx

echo [STEP 1] configure
bash -c "./configure --target=x86_64-win64-vs16 --enable-external-build --disable-examples --disable-tools --disable-docs --disable-unit-tests --disable-vp8-decoder --disable-vp8-encoder --disable-vp9-encoder --size-limit=640x360 --disable-install-bins --disable-install-srcs --disable-avx512 2>&1"
if errorlevel 1 (
  echo [FAIL] configure failed
  exit /b 1
)
echo [OK] configure done

echo [STEP 2] nmake
nmake > build_nmake.log 2>&1
if errorlevel 1 (
  echo [FAIL] nmake failed, tail of log:
  tail -40 build_nmake.log
  exit /b 1
)
echo [OK] nmake done
echo "=== LIB OUTPUT ==="
dir *.lib 2>nul
for %%f in (*.lib) do echo %%~nxf
