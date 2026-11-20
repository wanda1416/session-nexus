@echo off
setlocal
cd /d "%~dp0"

if "%~1"=="" goto run_tauri
if /I "%~1"=="--help" goto show_help
if /I "%~1"=="-h" goto show_help
if /I "%~1"=="--web" goto run_web
if /I "%~1"=="--vite" goto run_web
if /I "%~1"=="--test" goto run_test
if /I "%~1"=="--check" goto run_check
if /I "%~1"=="--build" goto run_build

:run_tauri
if not exist "node_modules" (
    echo [dev] node_modules not found, running pnpm install...
    call pnpm install
    if errorlevel 1 exit /b %ERRORLEVEL%
)
call pnpm tauri dev %*
exit /b %ERRORLEVEL%

:run_web
call pnpm dev %2 %3 %4 %5 %6 %7 %8 %9
exit /b %ERRORLEVEL%

:run_test
cargo test --manifest-path src-tauri/Cargo.toml --lib %2 %3 %4 %5 %6 %7 %8 %9
exit /b %ERRORLEVEL%

:run_check
call pnpm build
if errorlevel 1 exit /b %ERRORLEVEL%
cargo check --manifest-path src-tauri/Cargo.toml
exit /b %ERRORLEVEL%

:run_build
call pnpm tauri build %2 %3 %4 %5 %6 %7 %8 %9
exit /b %ERRORLEVEL%

:show_help
echo Usage: dev.bat [option] [args...]
echo.
echo Options:
echo   (default)       Start Tauri desktop app in dev mode (pnpm tauri dev)
echo   --web, --vite   Start frontend-only Vite dev server (pnpm dev)
echo   --test          Run Rust unit tests (cargo test --lib)
echo   --check         Run TypeScript/Vite build and Cargo check
echo   --build         Build Tauri release bundle (pnpm tauri build)
echo   -h, --help      Show this help message
exit /b 0
