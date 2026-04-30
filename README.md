# GrimMod: Mod Loader for Grim Fandango Remastered

Download at [https://hexagon.codes/grimhd](https://hexagon.codes/grimhd)

## Features

* **Mods**
  - Allows for the creation of asset mods that can swap out any file usually loaded from the game's .LAB datapacks.
* **High-Quality Assets**
  - Upgrades the renderer to support high-quality assets with any resolution and 32-bit color. Without this, assets are max 640x480 with dithered 24-bit color.
* **Forced VSync**
  - Previously, the remaster didn't use vsync while in-game and produced frames as fast as it could (sometimes causing coil whine).
* **High-DPI Fix** (Windows only)
  - On high-dpi systems with UI scaling above 100%, the remaster renders at a lower resolution and lets the system scale it up. This isn't that noticeable with 640x480 backgrounds but it stops high-quality assets from displaying at their full resolution. GrimMod forces the game to always render at the window's actual resolution.
* **Quick Renderer Toggle**
  - In the remaster, toggling the renderer between Original/Remastered is a smooth transition which makes changes less noticeable. GrimMod makes the toggle instant, highlighting the differences between the renderers.

## Installation

### Windows

1. Place `glu32.dll` in the root of the Grim Fandango Remastered directory where `GrimFandango.exe` is located.
2. (Optional) Put any mods in a `Mods` folder in the root directory.
3. Enjoy the ride!

Note: For GOG on Windows, when launching with the "Launch Grim Fandango Remastered" shortcut (like GOG Galaxy does), it's necessary to locate the shortcut in the game folder and set "Properties -> Compatibility -> Run this program as an administrator".

Note: For the Steam Deck, first set the game to run the Windows version by using Proton in the compatibility options.

### Linux (GOG)

1. Place `libgrimmod.so` in the game's `bin` directory alongside the `GrimFandango` executable (typically `<install-path>/game/bin/`).
2. Launch the game with `LD_PRELOAD` pointing to the library:
   ```bash
   cd <install-path>/game/bin
   LD_PRELOAD=./libgrimmod.so ./GrimFandango
   ```
   Alternatively, edit the GOG launch script (`start.sh`) and add the following line before the game is executed:
   ```bash
   export LD_PRELOAD="$bin_path/libgrimmod.so"
   ```
3. (Optional) Put any mods in a `Mods` folder inside the `bin` directory (i.e. `<install-path>/game/bin/Mods/`).
4. (Optional) Create a `grimmod.toml` in the `bin` directory to configure options (see [Config](#config)).

Note: When running the native Linux build under **box64** (e.g. on non-x86 hardware), use `BOX64_LD_PRELOAD` instead of `LD_PRELOAD`:
```bash
export BOX64_LD_PRELOAD="$bin_path/libgrimmod.so"
```

## Changelog
  ### 2.0.0
  - Added Linux support (native GOG Linux version via `LD_PRELOAD`).
  - Fixed CString memory leak in modded file open path.
  - Fixed struct layout for `RenderPassEntity` and `Draw` (renderer correctness).
  - Fixed off-by-one in `Vector<T>::len()`.
  - PNG loading on Linux uses lenient checksum validation (required for GrimHD mod assets).
  ### 1.1.0
  - Added GOG support (see installation note).
  - Added Steam Deck support (see installation note).
  - Improved initialization stability with better logging for errors.
  ### 1.0.0
  - Initial release.

## Limitations

* Doesn't allow for upscaling videos, yet. This includes full cutscenes and scenes that use video as part of the background/foreground (however most animations are simply a series of images, which can be upscaled).
* Doesn't attempt to make the game 16:9. Any non-4:3 assets will still look stretched.
* A full playthrough with grimmod has been completed but as new software, bugs and crashes are to be expected. Save regularly (but autosave is a potential future feature!).

## Config

Create a `grimmod.toml` file beside the game binary (`glu32.dll` on Windows, `GrimFandango` on Linux) to tweak options:

| Setting                               | Default          | Effect |
| ------------------------------------- | ---------------- | ------ |
| `mods = true/false`                   | true             | Enable/disable the loading of mods |
| `renderer.hq_assets = true/false`     | true             | Enable/disable hooking the renderer to load modern image formats (PNG/VP9 MKV) from mods |
| `renderer.quick_toggle = true/false`  | true             | Enable for instant toggling between the Original/Remastered renderers, disable to restore the smooth transition |
| `renderer.video_cutouts = true/false` | true             | Some scenes use videos, which are not yet upscalable with GrimMod, as the entire background image. This option allows GrimMod to manually carve out static chunks of the video, exposing the background underneath. As a somewhat hacky solution it has been given its own toggle if issues pop up. |
| `display.vsync = true/false`          | true             | Enable/disable forced VSync |
| `display.hdpi_fix = true/false`       | true (Win) / false (Linux) | Windows only. GrimMod rewrites some of the window handling to always render at native resolution. Not applicable on Linux. |
| `logging.enabled = true/false`        | true             | Enable/disable creation of and writing to `grimmod.log` with simple logging info, mostly for the purposes of a health check. |
| `logging.debug = true/false`          | false            | Enable/disable debug logging. This outputs a lot of information per frame, useless outside of debugging/development. |
| `logging.profile = true/false`        | false            | Enable/disable per-frame performance profiling. Logs hook timing data every 120 frames. |

## Building

The project requires Rust Nightly (uses `#![feature(fn_traits, tuple_trait, unboxed_closures)]`) and libvpx.

libvpx can be linked **statically** (recommended for distribution — produces a self-contained binary with no runtime dependencies) or **dynamically** (simpler for development but the target machine must have a matching libvpx installed). Static linking is controlled via the `VPX_STATIC=1` environment variable along with `VPX_LIB_DIR`, `VPX_INCLUDE_DIR`, and `VPX_VERSION`. Dynamic linking uses `pkg-config` to find libvpx automatically.

### Windows

```bash
cargo +nightly build --release --target i686-pc-windows-msvc
```

The output is `target/i686-pc-windows-msvc/release/grimmod.dll`, which should be renamed to `glu32.dll` for deployment.

### Linux

The Linux build targets 32-bit x86 (`i686-unknown-linux-gnu`) and requires a 32-bit C toolchain and 32-bit `libvpx`.

**Static linking (recommended)** — produces a self-contained `.so` with no libvpx runtime dependency:

First, install 32-bit build prerequisites. On Fedora:
```bash
sudo dnf install gcc glibc-devel.i686 libstdc++-devel.i686 nasm yasm
rustup toolchain install nightly
rustup +nightly target add i686-unknown-linux-gnu
```

On Debian/Ubuntu:
```bash
sudo apt-get install gcc-multilib libc6-dev-i386 nasm yasm
rustup toolchain install nightly
rustup +nightly target add i686-unknown-linux-gnu
```

Build a 32-bit static libvpx (decoder-only — encoders are not needed and avoids C++ multilib issues):
```bash
git clone https://chromium.googlesource.com/webm/libvpx
cd libvpx && git checkout v1.16.0
mkdir build-i686 && cd build-i686
CFLAGS='-m32' LDFLAGS='-m32' CC='gcc -m32' \
  ../configure --target=x86-linux-gcc \
  --enable-static --disable-shared \
  --enable-vp8-decoder --disable-vp8-encoder \
  --enable-vp9-decoder --disable-vp9-encoder \
  --disable-examples --disable-tools --disable-docs --disable-unit-tests \
  --prefix="$(pwd)/install"
make -j$(nproc) && make install
```

Then build grimmod:
```bash
VPX_LIB_DIR=<path-to>/build-i686/install/lib \
  VPX_INCLUDE_DIR=<path-to>/build-i686/install/include \
  VPX_VERSION=1.16.0 \
  VPX_STATIC=1 \
  cargo +nightly build --release --target i686-unknown-linux-gnu
```

**Dynamic linking** — links against system libvpx (the target machine must have 32-bit libvpx installed). Using Docker:

```bash
docker run --rm --platform linux/amd64 -v "$(pwd)":/src -w /src rust:latest bash -c "
  dpkg --add-architecture i386 &&
  apt-get update &&
  apt-get install -y gcc-multilib libc6-dev-i386 libvpx-dev:i386 pkg-config:i386 &&
  rustup toolchain install nightly &&
  rustup +nightly target add i686-unknown-linux-gnu &&
  export PKG_CONFIG_ALLOW_CROSS=1 &&
  export PKG_CONFIG_PATH=/usr/lib/i386-linux-gnu/pkgconfig &&
  cargo +nightly build --release --target i686-unknown-linux-gnu
"
```

The output is `target/i686-unknown-linux-gnu/release/libgrimmod.so`.

## Architecture

GrimMod works by hooking game functions at runtime to intercept asset loading and rendering calls.

- **Windows**: Injected as a DLL proxy (`glu32.dll`). Uses `retour` for inline function hooking, IAT overwriting for indirect hooks, and `lightningscanner` for byte-pattern scanning to locate game functions.
- **Linux**: Injected via `LD_PRELOAD`. Uses inline x86 prologue patching with mmap'd trampolines for direct hooks, GOT overwriting for indirect hooks, and ELF symtab parsing (via `goblin`) to resolve game symbols.
