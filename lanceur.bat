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
echo  6. Verifier le demarrage (sans fenetre)
echo  0. Quitter
echo.
set "choix="
set /p "choix=Votre choix : "
if not defined choix exit /b 0

if "%choix%"=="1" goto lancer_debug
if "%choix%"=="2" goto lancer_release
if "%choix%"=="3" goto lancer_tests
if "%choix%"=="4" goto valider
if "%choix%"=="5" goto compiler_release
if "%choix%"=="6" goto verifier_demarrage
if "%choix%"=="0" exit /b 0
echo Choix invalide. Saisissez un numero du menu.
pause
goto menu

:lancer_debug
call :executer cargo run
goto menu

:lancer_release
call :executer cargo run --release
goto menu

:lancer_tests
call :executer cargo test --all-targets
goto menu

:valider
call :validation
goto menu

:compiler_release
call :executer cargo build --release
goto menu

:verifier_demarrage
call :executer cargo run -- --check
goto menu

rem Execute la commande passee en arguments puis attend une touche pour revenir au menu.
:executer
echo.
%*
set "code=%errorlevel%"
echo.
echo Code de sortie : %code%
pause
goto :eof

rem Verifie fmt, clippy strict et tous les tests ; s'arrete au premier echec.
:validation
echo.
echo [1/3] Verification du formatage...
cargo fmt --all -- --check
set "code=%errorlevel%"
if not "%code%"=="0" goto echec_validation
echo [2/3] Clippy strict...
cargo clippy --all-targets -- -D warnings
set "code=%errorlevel%"
if not "%code%"=="0" goto echec_validation
echo [3/3] Tests de toutes les cibles...
cargo test --all-targets
set "code=%errorlevel%"
if not "%code%"=="0" goto echec_validation
echo.
echo Validation reussie.
pause
goto :eof

:echec_validation
echo.
echo Echec de la validation (code %errorlevel%).
pause
goto :eof
