@echo off
rem Builds hiddriver_1.dll and hiddriver_2.dll (32-bit, like the official app).
rem Needs Visual Studio with the C++ workload; finds it with vswhere.
setlocal
cd /d "%~dp0"
if defined VSCMD_VER goto build
"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -find VC\Auxiliary\Build\vcvars32.bat > "%TEMP%\shark_vcvars.txt" || exit /b 1
set /p VCVARS=<"%TEMP%\shark_vcvars.txt"
call "%VCVARS%" >nul || exit /b 1

:build
for %%N in (1 2) do (
  cl /nologo /O2 /LD /DDRV=%%N shim.c /Fe:hiddriver_%%N.dll /link /DEF:hiddriver_%%N.def || exit /b 1
)
del /q shim.obj *.exp *.lib 2>nul
