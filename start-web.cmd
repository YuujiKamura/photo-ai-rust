@echo off
setlocal

set ROOT=%~dp0
set WEB_DIR=%ROOT%web
set SERVER_EXE=%WEB_DIR%\webserver.exe

powershell -NoProfile -Command "try { Invoke-WebRequest -UseBasicParsing http://localhost:9998/api/job | Out-Null; exit 0 } catch { exit 1 }"
if errorlevel 1 (
  if not exist "%SERVER_EXE%" (
    pushd "%WEB_DIR%"
    "C:\Program Files\Go\bin\go.exe" build -o "%SERVER_EXE%" server.go
    if errorlevel 1 exit /b 1
    popd
  )
  start "photo-ai webserver" /D "%WEB_DIR%" "%SERVER_EXE%"
  powershell -NoProfile -Command "Start-Sleep -Seconds 2"
)

start "" "http://localhost:9998"
endlocal
