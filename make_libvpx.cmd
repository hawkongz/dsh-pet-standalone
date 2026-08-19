@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 (
  echo [FAIL] vcvars64 failed
  exit /b 1
)
echo [OK] vcvars loaded
bash -c "export PATH='/c/allsoftware/devs/ds-pet/tools_msys/usr/bin:/c/Program Files/Git/usr/bin:$PATH'; cd /c/allsoftware/devs/ds-pet/vendor_libvpx; make -j8 target=libs libs > make_build.log 2>&1; ec=$?; tail -15 make_build.log; echo EXITCODE=$ec; exit $ec"
