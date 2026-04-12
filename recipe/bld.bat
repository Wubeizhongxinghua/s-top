@echo on

cargo install --locked --path . --root "%LIBRARY_PREFIX%"
if exist "%LIBRARY_PREFIX%\\.crates.toml" del /f /q "%LIBRARY_PREFIX%\\.crates.toml"
if exist "%LIBRARY_PREFIX%\\.crates2.json" del /f /q "%LIBRARY_PREFIX%\\.crates2.json"
