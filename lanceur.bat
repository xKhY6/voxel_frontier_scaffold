@echo off
rem Code page UTF-8 pour afficher correctement les accents du menu.
chcp 65001 >nul
setlocal
rem Se place a la racine du projet, quel que soit le dossier d'ou le fichier est lance.
cd /d "%~dp0"

rem Ajoute rustup au PATH de la session si cargo n'y est pas (installation utilisateur sans modification du PATH).
where cargo >nul 2>&1 || set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where cargo >nul 2>&1 || (echo Cargo introuvable. Installez Rust via rustup : https://rustup.rs & pause & exit /b 1)

:menu
cls
echo ==========================================
echo   Voxel Frontier - Lanceur
echo ==========================================
echo  1. Lancer le jeu (debug)
echo  2. Lancer le jeu (release)
echo  3. Lancer les tests
echo  4. Validation complete (fmt, clippy, tests)
echo  5. Compiler en release
echo  0. Quitter
echo.
set "choix="
set /p "choix=Votre choix : "

if "%choix%"=="1" call :executer cargo run
if "%choix%"=="2" call :executer cargo run --release
if "%choix%"=="3" call :executer cargo test
if "%choix%"=="4" call :validation
if "%choix%"=="5" call :executer cargo build --release
if "%choix%"=="0" exit /b 0
goto menu

rem Execute la commande passee en arguments puis attend une touche pour revenir au menu.
:executer
echo.
%*
echo.
echo Code de sortie : %errorlevel%
pause
goto :eof

rem Enchaine fmt, clippy et tests ; s'arrete au premier echec.
:validation
echo.
echo [1/3] Formatage...
cargo fmt
if errorlevel 1 goto echec_validation
echo [2/3] Clippy...
cargo clippy --all-targets
if errorlevel 1 goto echec_validation
echo [3/3] Tests...
cargo test
if errorlevel 1 goto echec_validation
echo.
echo Validation reussie.
pause
goto :eof

:echec_validation
echo.
echo Echec de la validation.
pause
goto :eof
