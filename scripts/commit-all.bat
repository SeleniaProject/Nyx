@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM Commit each pending change as a separate commit in the current git repo.
REM Usage: commit-all.bat [--push] [--sign] [--prefix "feat|fix|docs|chore(...)"]
REM - The script derives an action (add/update/delete/rename) and uses the file path in the message.
REM - Order is taken from `git status --porcelain` output; to control order, stage files in advance.

set "DO_PUSH=0"
set "DO_SIGN=0"
set "PREFIX=chore"

REM Parse arguments
:parse_args
if "%~1"=="" goto :args_done
if /I "%~1"=="--push" set "DO_PUSH=1"& shift & goto :parse_args
if /I "%~1"=="--sign" set "DO_SIGN=1"& shift & goto :parse_args
if /I "%~1"=="--prefix" set "PREFIX=%~2"& shift & shift & goto :parse_args
echo Unknown argument: %~1
exit /b 1
:args_done

REM Ensure we are inside a git repo
git rev-parse --is-inside-work-tree >nul 2>&1
if errorlevel 1 (
  echo [ERROR] Not inside a git repository.
  exit /b 1
)

REM Collect pending changes
for /f "delims=" %%L in ('git status --porcelain') do (
  set "LINE=%%L"
  if "!LINE!"=="" goto :continue

  REM Extract status (first 2 chars) and path (after a space)
  set "STATUS=!LINE:~0,2!"
  set "RAWPATH=!LINE:~3!"

  REM Handle rename lines in porcelain: e.g., "R  old/path -> new/path"
  set "PATH=!RAWPATH!"
  echo !RAWPATH! | findstr /C:" -> " >nul
  if not errorlevel 1 (
    for /f "tokens=2 delims=>" %%P in ("!RAWPATH!") do set "PATH=%%P"
    REM Trim leading spaces
    for /f "tokens=*" %%Q in ("!PATH!") do set "PATH=%%Q"
  )

  REM Determine action for commit message
  set "ACTION=update"
  if "!STATUS!"=="A " set "ACTION=add"
  if "!STATUS!"=="??" set "ACTION=add"
  if "!STATUS!"=="D " set "ACTION=delete"
  if "!STATUS:~0,1!"=="R" set "ACTION=rename"

  REM Stage the file (or removal)
  if "!ACTION!"=="delete" (
    git rm -- "!PATH!" >nul 2>&1
  ) else (
    git add -- "!PATH!" >nul 2>&1
  )

  if errorlevel 1 (
    echo [WARN] Skipping "!PATH!" (failed to stage)
    goto :continue
  )

  REM Build commit message
  set "MSG=%PREFIX%: !ACTION! !PATH!"
  if %DO_SIGN%==1 (
    git commit -S -m "!MSG!" >nul 2>&1
  ) else (
    git commit -m "!MSG!" >nul 2>&1
  )

  if errorlevel 1 (
    echo [WARN] Commit failed for "!PATH!"; continuing.
  ) else (
    echo [OK] Committed: !MSG!
  )

  :continue
)

REM Optionally push
if %DO_PUSH%==1 (
  git rev-parse --abbrev-ref --symbolic-full-name @{u} >nul 2>&1
  if errorlevel 1 (
    echo [INFO] No upstream configured; skipping push.
  ) else (
    git push
  )
)

endlocal
exit /b 0


