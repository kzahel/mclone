@echo off
setlocal EnableExtensions

set "REPO_ROOT=%~dp0.."
for %%I in ("%REPO_ROOT%") do set "REPO_ROOT=%%~fI"

set "GIT_BASH=%MCLONE_GIT_BASH%"
if not defined GIT_BASH set "GIT_BASH=%MCLONE_BASH%"
if not defined GIT_BASH if defined ProgramFiles if exist "%ProgramFiles%\Git\bin\bash.exe" set "GIT_BASH=%ProgramFiles%\Git\bin\bash.exe"
if not defined GIT_BASH if defined ProgramW6432 if exist "%ProgramW6432%\Git\bin\bash.exe" set "GIT_BASH=%ProgramW6432%\Git\bin\bash.exe"
if not defined GIT_BASH if defined ProgramFiles(x86) if exist "%ProgramFiles(x86)%\Git\bin\bash.exe" set "GIT_BASH=%ProgramFiles(x86)%\Git\bin\bash.exe"
if not defined GIT_BASH if defined LocalAppData if exist "%LocalAppData%\Programs\Git\bin\bash.exe" set "GIT_BASH=%LocalAppData%\Programs\Git\bin\bash.exe"
if not defined GIT_BASH for /f "delims=" %%B in ('where bash.exe 2^>nul') do (
    if not defined GIT_BASH set "GIT_BASH=%%B"
)

set "PAUSE_ON_ERROR=1"
set "FORWARDED_ARGS="

:parse_args
if "%~1"=="" goto args_done
if /I "%~1"=="-h" goto usage
if /I "%~1"=="--help" goto usage
if /I "%~1"=="/?" goto usage
if /I "%~1"=="--no-pause" (
    set "PAUSE_ON_ERROR=0"
    shift
    goto parse_args
)
if /I "%~1"=="-NoPause" (
    set "PAUSE_ON_ERROR=0"
    shift
    goto parse_args
)
if /I "%~1"=="--pause-on-error" (
    set "PAUSE_ON_ERROR=1"
    shift
    goto parse_args
)
if "%~1"=="--" (
    shift
    goto copy_remaining_args
)
set "FORWARDED_ARGS=%FORWARDED_ARGS% "%~1""
shift
goto parse_args

:copy_remaining_args
if "%~1"=="" goto args_done
set "FORWARDED_ARGS=%FORWARDED_ARGS% "%~1""
shift
goto copy_remaining_args

:usage
echo Usage: scripts\start-android-xr.bat [script-options] [android-xr-options]
echo.
echo Refreshes the Minecraft asset pack, builds and installs the Quest Android XR
echo APK, stages assets, wakes the attached Quest, and launches Mclone XR for
echo interactive testing. The app keeps running after this script exits.
echo.
echo Default delegated command:
echo   bash android-xr/start-quest-openxr.sh [android-xr-options]
echo.
echo Script options:
echo   --no-pause             Do not pause if install or launch fails.
echo   --pause-on-error       Pause before closing on failures. This is the default.
echo   -h, --help, /?         Show this help.
echo.
echo Common Android XR options:
echo   --debug                Build/install the debug APK instead of release.
echo   --serial DEVICE_SERIAL Use a specific attached Quest.
echo   --skip-build           Reuse the existing APK.
echo   --skip-asset-refresh   Reuse the existing extracted.zip pack.
echo   --view-pose X,Y,Z,YAW  Set the startup XR view pose.
echo.
echo Set MCLONE_GIT_BASH or MCLONE_BASH to override Git Bash discovery.
exit /b 0

:args_done
if not defined GIT_BASH (
    echo Missing required command: Git Bash.
    echo Install Git for Windows, or set MCLONE_GIT_BASH to the full path of bash.exe.
    set "STATUS=1"
    goto exit_with_status
)
if not exist "%GIT_BASH%" (
    echo Git Bash was not found:
    echo   %GIT_BASH%
    echo Install Git for Windows, or set MCLONE_GIT_BASH to the full path of bash.exe.
    set "STATUS=1"
    goto exit_with_status
)

echo Starting interactive Mclone Android XR on the attached Quest headset...
echo Git Bash: %GIT_BASH%
echo Repo: %REPO_ROOT%
echo Command: bash android-xr/start-quest-openxr.sh%FORWARDED_ARGS%

pushd "%REPO_ROOT%"
if errorlevel 1 (
    set "STATUS=1"
    goto exit_with_status
)
"%GIT_BASH%" -lc "bash android-xr/start-quest-openxr.sh ""$@""" mclone-android-xr-start %FORWARDED_ARGS%
set "STATUS=%ERRORLEVEL%"
popd
goto exit_with_status

:exit_with_status
if "%STATUS%"=="" set "STATUS=1"
if not "%STATUS%"=="0" (
    echo.
    echo Android XR Quest setup or launch failed with status %STATUS%.
    echo If the headset is not connected or authorized, connect it, accept USB debugging in-headset, and retry.
    if not "%PAUSE_ON_ERROR%"=="0" (
        echo.
        pause
    )
)
exit /b %STATUS%
