!#/usr/bin/sh
which wasm-bindgen python3 || { echo "python3 and wasm-bindgen is required"; exit 1; }
cargo build --target wasm32-unknown-unknown
wasm-bindgen --out-dir web --target web ./target/wasm32-unknown-unknown/debug/flying-dragon.wasm
python3 -m http.server --directory web
