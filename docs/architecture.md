# Runtime architecture

`ok-script` has one framework-neutral runtime and optional UI adapters.

```
ok/core/          events, task discovery, startup, screenshots, downloads, translations
ok/task/          automation task and executor logic
ok/device/        capture and interaction logic
ok/ui/qt/         PySide6 and PySide6-Fluent-Widgets desktop UI
ok/ui/web/        FastAPI API and HTML/JavaScript browser UI
ok/ui/gpui/       Native GPUI client and the shared-Web-API launcher
ok/gui/           compatibility namespace for existing imports
```

Core modules publish through `ok.core.events.communicate`. An event signal has a
small `connect`, `disconnect`, and `emit` API without depending on a UI toolkit.
The Qt adapter installs a main-thread dispatcher; the web adapter subscribes to
the same stream and forwards serializable events over a WebSocket. The GPUI
adapter starts the same local FastAPI server, then launches the native Rust
client against that server. This keeps task state, actions, custom-tab
operations, and runtime events in one contract; Qt remains an independent
frontend.

## Install a mode

```bash
pip install ok-script                # minimal headless core
pip install "ok-script[default]"     # headless runtime profile
pip install "ok-script[qt]"          # desktop UI
pip install "ok-script[web]"         # FastAPI/web UI
pip install "ok-script[gpui]"        # native GPUI shell (client binary required)
pip install "ok-script[adb]"         # ADB devices
pip install "ok-script[ocr]"         # ONNX OCR
```

OpenCV is not part of any profile. Users must choose the version and exactly one
suitable variant for their application: `opencv-python`,
`opencv-contrib-python`, `opencv-python-headless`, or
`opencv-contrib-python-headless`.

The optional-dependency profiles and local TOML dependency groups have identical
names and requirements. For headless repository development, install:

```bash
pip install --editable . --group default --group dev
```

Add the `web`, `gpui`, `qt`, `adb`, or `ocr` group for the use case being developed.
The GPUI crate is under `ok/ui/gpui` and tracks the Zed upstream `main` branch;
`Cargo.lock` pins the currently selected upstream commit. The checked-in
`rust-toolchain.toml` selects the nightly toolchain required by that upstream
revision. Build it from the crate directory with:

```bash
cargo build --release --locked
```

Either put the resulting `ok-script-gpui.exe` beside the Python executable or
configure `gui.binary`/`OK_SCRIPT_GPUI_BINARY`.

## Run a mode

```bash
ok run_task DailyTask --config src.config:config
ok gui --config src.config:config
ok web --config src.config:config --host 127.0.0.1 --port 8000
ok gpui --config src.config:config
```

The web server defaults to loopback. Passing a public host such as `0.0.0.0`
exposes task controls to the network, so authentication and a reverse proxy
should be added before doing that on an untrusted network.

The browser UI source is a React + TypeScript + Vite project under the
repository-level `web_src` folder and uses `@fluentui/react-components`.
Only its compiled output is stored under `ok/ui/web/static`, so those runtime
assets are included in the Python wheel. Rebuild them after frontend changes:

```bash
npm install
npm run dev
npm run build
```

Both commands run from the repository root. They regenerate the web translation
catalog from the Qt `.ts` files before starting Vite or producing the packaged
assets.

Application-specific browser pages can be declared by registered tasks without
depending on FastAPI or accessing the executor directly. See
[task-backed web tabs](en/task_web_tabs.md).

New desktop imports should use `ok.ui.qt`. Existing `ok.gui` imports remain
available as a migration shim.
