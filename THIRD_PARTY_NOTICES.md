# Third-party notices

Querybook (the `.app` and `.dmg` releases, and this source tree) includes or links
the components below. Each keeps its own license; this file reproduces the notices
those licenses require.

## SQLCipher 4.19.0 (Community Edition)

Statically linked into the app through `rusqlite` / `libsqlite3-sys` (`bundled-sqlcipher`).
<https://www.zetetic.net/sqlcipher/>

```
Copyright (c) 2025, ZETETIC LLC
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:
    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright
      notice, this list of conditions and the following disclaimer in the
      documentation and/or other materials provided with the distribution.
    * Neither the name of the ZETETIC LLC nor the
      names of its contributors may be used to endorse or promote products
      derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY ZETETIC LLC ''AS IS'' AND ANY
EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL ZETETIC LLC BE LIABLE FOR ANY
DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## SQLite 3.53.4

Part of the SQLCipher build above. SQLite is in the public domain
(<https://www.sqlite.org/copyright.html>).

## Bootstrap 5.3.3 and Bootstrap Icons 1.11.3

Vendored in `assets/vendor/` and shipped inside the app.
<https://getbootstrap.com/> · <https://icons.getbootstrap.com/>

```
The MIT License (MIT)

Copyright (c) 2011-2024 The Bootstrap Authors   (Bootstrap)
Copyright (c) 2019-2024 The Bootstrap Authors   (Bootstrap Icons)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

## Rust crates

The app is built from Rust crates published on crates.io, almost all under
`MIT OR Apache-2.0` (some MIT, BSD or Zlib). The main ones:

| Crate | Used for | License |
|---|---|---|
| tauri, tauri-build, tauri-plugin-dialog | desktop shell, windows, native dialogs | MIT OR Apache-2.0 |
| leptos, leptos_router | user interface | MIT |
| rusqlite, libsqlite3-sys | SQLite / SQLCipher access | MIT |
| csv | CSV import / export | Unlicense OR MIT |
| serde, serde_json, serde-wasm-bindgen | serialization | MIT OR Apache-2.0 / MIT |
| wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys | WebAssembly ↔ browser bindings | MIT OR Apache-2.0 |
| gloo-timers, console_error_panic_hook | timers, panic messages | MIT OR Apache-2.0 |

Each crate's full license text ships with its source on crates.io. The complete
dependency list is in `Cargo.lock`.
