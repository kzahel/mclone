@echo off
rem Patterned after ..\..\playbox\scripts\start-xr.bat. Keep this as a thin
rem Windows entry point over start-xr.ps1 so Quest wake/restore behavior stays
rem centralized in the PowerShell launcher/module.
setlocal EnableExtensions

set "SCRIPT_DIR=%~dp0"
set "PS_SCRIPT=%SCRIPT_DIR%start-xr.ps1"
set "PAUSE_ON_ERROR=1"
set "PS_ARGS="
set "VIEW_POSE_ARG="

:parse_args
if "%~1"=="" goto args_done
if /I "%~1"=="--release" (
    call :append_arg "-Release"
    shift
    goto parse_args
)
if /I "%~1"=="-Release" (
    call :append_arg "-Release"
    shift
    goto parse_args
)
if /I "%~1"=="--build-only" (
    call :append_arg "-BuildOnly"
    shift
    goto parse_args
)
if /I "%~1"=="-BuildOnly" (
    call :append_arg "-BuildOnly"
    shift
    goto parse_args
)
if /I "%~1"=="--check-only" (
    call :append_arg "-CheckOnly"
    shift
    goto parse_args
)
if /I "%~1"=="-CheckOnly" (
    call :append_arg "-CheckOnly"
    shift
    goto parse_args
)
if /I "%~1"=="--runtime" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Runtime"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="-Runtime" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Runtime"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--runtime-json" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Runtime"
    call :append_arg "json"
    call :append_arg "-RuntimeJson"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="-RuntimeJson" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-RuntimeJson"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--vdxr" goto runtime_virtual_desktop
if /I "%~1"=="--virtual-desktop" goto runtime_virtual_desktop
if /I "%~1"=="--active-runtime" goto runtime_active
if /I "%~1"=="--system-runtime" goto runtime_active
if /I "%~1"=="--environment-runtime" goto runtime_environment
if /I "%~1"=="--smoke" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Smoke"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="-Smoke" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Smoke"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--mclone" (
    call :append_arg "-Smoke"
    call :append_arg "mclone"
    shift
    goto parse_args
)
if /I "%~1"=="--clear" (
    call :append_arg "-Smoke"
    call :append_arg "clear"
    shift
    goto parse_args
)
if /I "%~1"=="--frames" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Frames"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="-Frames" (
    call :require_value "%~1" "%~2" || goto exit_with_status
    call :append_arg "-Frames"
    call :append_arg "%~2"
    shift
    shift
    goto parse_args
)
if /I "%~1"=="--view-pose" goto parse_view_pose
if /I "%~1"=="--xr-view-pose" goto parse_view_pose
if /I "%~1"=="-ViewPose" goto parse_view_pose
if /I "%~1"=="--no-quest-launch" (
    call :append_arg "-NoQuestLaunch"
    shift
    goto parse_args
)
if /I "%~1"=="-NoQuestLaunch" (
    call :append_arg "-NoQuestLaunch"
    shift
    goto parse_args
)
if /I "%~1"=="--no-quest-restore" (
    call :append_arg "-NoQuestRestore"
    shift
    goto parse_args
)
if /I "%~1"=="-NoQuestRestore" (
    call :append_arg "-NoQuestRestore"
    shift
    goto parse_args
)
if /I "%~1"=="--no-system-panel-dismiss" (
    call :append_arg "-NoSystemPanelDismiss"
    shift
    goto parse_args
)
if /I "%~1"=="-NoSystemPanelDismiss" (
    call :append_arg "-NoSystemPanelDismiss"
    shift
    goto parse_args
)
if /I "%~1"=="--no-virtual-desktop" (
    call :append_arg "-NoVirtualDesktop"
    shift
    goto parse_args
)
if /I "%~1"=="-NoVirtualDesktop" (
    call :append_arg "-NoVirtualDesktop"
    shift
    goto parse_args
)
if /I "%~1"=="--allow-unsupported" (
    call :append_arg "-AllowUnsupported"
    shift
    goto parse_args
)
if /I "%~1"=="-AllowUnsupported" (
    call :append_arg "-AllowUnsupported"
    shift
    goto parse_args
)
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
if /I "%~1"=="-h" goto usage
if /I "%~1"=="--help" goto usage
if "%~1"=="--" (
    shift
    goto copy_remaining_args
)
call :append_arg "%~1"
shift
goto parse_args

:parse_view_pose
if "%~2"=="" (
    echo %~1 requires X,Y,Z,YAW_DEGREES.
    set "STATUS=1"
    goto exit_with_status
)
set "VIEW_POSE_FIRST=%~2"
if not "%VIEW_POSE_FIRST:,=%"=="%VIEW_POSE_FIRST%" (
    set "VIEW_POSE_ARG=%~2"
    shift
    shift
    goto parse_args
)
if "%~5"=="" (
    echo %~1 requires X,Y,Z,YAW_DEGREES.
    set "STATUS=1"
    goto exit_with_status
)
set "VIEW_POSE_ARG=%~2,%~3,%~4,%~5"
shift
shift
shift
shift
shift
goto parse_args

:runtime_virtual_desktop
call :append_arg "-Runtime"
call :append_arg "virtual-desktop"
shift
goto parse_args

:runtime_active
call :append_arg "-Runtime"
call :append_arg "active"
shift
goto parse_args

:runtime_environment
call :append_arg "-Runtime"
call :append_arg "environment"
shift
goto parse_args

:copy_remaining_args
if "%~1"=="" goto args_done
call :append_arg "%~1"
shift
goto copy_remaining_args

:usage
echo Usage: scripts\start-xr.bat [script-options] [mclone-xr-options]
echo.
echo Starts the native mclone desktop OpenXR smoke through start-xr.ps1.
echo.
echo Script options:
echo   --release              Run the optimized release build.
echo   --build-only           Build the XR binary without launching it.
echo   --check-only           Check the XR feature without building an executable.
echo   --runtime MODE         virtual-desktop, active, json, or environment.
echo   --runtime-json PATH    Runtime manifest to use with --runtime json.
echo   --vdxr                 Alias for --runtime virtual-desktop.
echo   --active-runtime       Alias for --runtime active.
echo   --smoke clear^|mclone   Select the smoke mode. Default: clear.
echo   --mclone               Alias for --smoke mclone.
echo   --frames N             Set the XR smoke frame budget. Default: 120.
echo   --view-pose X,Y,Z,YAW  Map headset startup pose to mclone world pose.
echo   --no-quest-launch      Reuse an already-connected headset session.
echo   --no-quest-restore     Leave Quest wake/proximity state untouched after run.
echo   --no-system-panel-dismiss
echo                          Do not dismiss known Quest system panels before launch.
echo   --no-pause             Do not pause if the launch fails.
echo   --pause-on-error       Pause before closing on failures. This is the default.
echo   -h, --help             Show this help.
echo.
echo Examples:
echo   scripts\start-xr.bat --check-only --runtime environment
echo   scripts\start-xr.bat --vdxr --frames 2
echo   scripts\start-xr.bat --vdxr --mclone --view-pose 0,78,-96,180 --frames 120
exit /b 0

:args_done
where powershell >nul 2>nul
if errorlevel 1 (
    echo Missing required command: powershell.
    set "STATUS=1"
    goto exit_with_status
)

if not exist "%PS_SCRIPT%" (
    echo Missing PowerShell launcher: %PS_SCRIPT%
    set "STATUS=1"
    goto exit_with_status
)

echo Starting mclone XR launcher...
if defined VIEW_POSE_ARG (
    echo Command: powershell -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%"%PS_ARGS% "-ViewPose" "%VIEW_POSE_ARG%"
    powershell -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" %PS_ARGS% -ViewPose "%VIEW_POSE_ARG%"
) else (
    echo Command: powershell -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%"%PS_ARGS%
    powershell -NoProfile -ExecutionPolicy Bypass -File "%PS_SCRIPT%" %PS_ARGS%
)
set "STATUS=%ERRORLEVEL%"
goto exit_with_status

:append_arg
set "PS_ARGS=%PS_ARGS% "%~1""
exit /b 0

:require_value
if "%~2"=="" (
    echo %~1 requires a value.
    set "STATUS=1"
    exit /b 1
)
exit /b 0

:exit_with_status
if "%STATUS%"=="" set "STATUS=1"
if not "%STATUS%"=="0" (
    echo.
    echo mclone XR launcher failed with status %STATUS%.
    echo If the headset or OpenXR runtime is not connected, connect it and retry.
    if not "%PAUSE_ON_ERROR%"=="0" (
        echo.
        pause
    )
)
exit /b %STATUS%
