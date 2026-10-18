$ErrorActionPreference = 'Stop'
$env:PYTHONUTF8 = '1'
Set-Location (Join-Path $PSScriptRoot '..')
function Invoke-Checked([scriptblock]$Task) {
    & $Task
    if ($LASTEXITCODE -ne 0) { throw "Command failed: $Task" }
}
Invoke-Checked { python tools/prepare-plugin-sdk.py }
$env:RUSTC = (rustup which --toolchain stable rustc)
$env:RUSTDOC = (rustup which --toolchain stable rustdoc)
Invoke-Checked { rustup target add --toolchain stable wasm32-unknown-unknown x86_64-pc-windows-msvc }
Invoke-Checked { rustup run stable cargo build --locked --release --lib --target wasm32-unknown-unknown }
Copy-Item target/wasm32-unknown-unknown/release/lxr02.wasm web/lxr02.wasm -Force
Invoke-Checked { python tools/package-plugin-ui.py }
$env:RUSTFLAGS = '-C target-feature=+crt-static'
Invoke-Checked { rustup run stable cargo build --locked --release --manifest-path plugin/engine/Cargo.toml --target x86_64-pc-windows-msvc }
$taskLibrary = Join-Path (Get-Location) 'plugin/engine/target/x86_64-pc-windows-msvc/release/lxr02_plugin.lib'
Invoke-Checked { cmake -S plugin -B target/plugin-build-win -G 'Visual Studio 17 2022' -A x64 "-DLXR_RUST_LIBRARY=$taskLibrary" }
Invoke-Checked { cmake --build target/plugin-build-win --config Release --parallel 4 }
Invoke-Checked { & (Join-Path (Get-Location) 'target/plugin-build-win/Release/dxm088-kit-store-tests.exe') }
Invoke-Checked { python tools/package-windows-release.py }
