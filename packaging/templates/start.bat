@echo off
setlocal enabledelayedexpansion

set "ROOT=%~dp0"
set "DATA_DIR=%ROOT%data"
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

set "FUNDVAL_DATA_DIR=%DATA_DIR%"
if "%SECRET_KEY%"=="" set "SECRET_KEY=django-insecure-dev-only"

set "BACKEND_PORT=%BACKEND_PORT%"
if "%BACKEND_PORT%"=="" set "BACKEND_PORT=8001"
set "FRONTEND_PORT=%FRONTEND_PORT%"
if "%FRONTEND_PORT%"=="" set "FRONTEND_PORT=3000"

rem Start backend in a new window
set "PORT=%BACKEND_PORT%"
start "fundval-backend" /D "%ROOT%" "%ROOT%fundval-backend.exe"

rem Start frontend (blocks current window)
set "PORT=%FRONTEND_PORT%"
set "API_PROXY_TARGET=http://localhost:%BACKEND_PORT%"
"%ROOT%node\\node.exe" "%ROOT%frontend\\server.js"

