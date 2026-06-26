@echo off
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
set "START_XR=%SCRIPT_DIR%start-xr.bat"

if not exist "%START_XR%" (
    echo Missing desktop XR launcher: %START_XR%
    exit /b 1
)

echo Starting interactive mclone desktop OpenXR...
echo Command: "%START_XR%" --vdxr --mclone --forever %*
call "%START_XR%" --vdxr --mclone --forever %*
exit /b %ERRORLEVEL%
