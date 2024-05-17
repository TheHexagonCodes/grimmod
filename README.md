# GrimMod: Mod Loader for Grim Fandango Remastered

Download at [https://hexagon.codes/grimhd](https://hexagon.codes/grimhd)

## Features

* **Mods**
  - Allows for the creation of asset mods that can swap out any file usually loaded from the game's .LAB datapacks.
* **High-Quality Assets**
  - Upgrades the renderer to support high-quality assets with any resolution and 32-bit color. Without this, assets are max 640x480 with dithered 24-bit color.
* **Forced VSync**
  - Previously, the remaster didn't use vsync while in-game and produced frames as fast as it could (sometimes causing coil whine).
* **High-DPI Fix**
  - On high-dpi systems with UI scaling above 100%, the remaster renders at a lower resolution and lets the system scale it up. This isn't that noticeable with 640x480 backgrounds but it stops high-quality assets from displaying at their full resolution. GrimMod forces the game to always render at the window's actual resolution.
* **Quick Renderer Toggle**
  - In the remaster, toggling the renderer between Original/Remastered is a smooth transition which makes changes less noticeable. GrimMod makes the toggle instant, highlighting the differences between the renderers.

## Installation

1. Place `glu32.dll` in the root of the Grim Fandango Remastered directory where `GrimFandango.exe` is located.
2. (Optional) Put any mods in a `Mods` folder in the root directory.
3. Enjoy the ride!

## Limitations

* Doesn't allow for upscaling videos, yet. This includes full cutscenes and scenes that use video as part of the background/foreground (however most animations are simply a series of images, which can be upscaled).
* Doesn't attempt to make the game 16:9. Any non-4:3 assets will still look stretched.
* A full playthrough with grimmod has been completed but as new software, bugs and crashes are to be expected. Save regularly (but autosave is a potential future feature!).
* Doesn't work on Steam Deck (or on the one tested at least). Even without GrimMod, the remaster crashes under Proton. Should be fixable.

## Config

By creating a `grimmod.toml` file beside `glu32.dll`, some options can be tweaked:

| Setting                               | Default | Effect |
| ------------------------------------- | ------- | ------ |
| `mods = true/false`                   | true    | Enable/disable the loading of mods |
| `renderer.hq_assets = true/false`     | true    | Enable/disable hooking the renderer to load modern image formats (PNG/VP9 MKV) from mods |
| `renderer.quick_toggle = true/false`  | true    | Enable for instant toggling between the Original/Remastered renderers, disable to restore the smooth transition |
| `renderer.video_cutouts = true/false` | true    | Some scenes use videos, which are not yet upscalable with GrimMod, as the entire background image. This option allows GrimMod to manually carve out static chunks of the video, exposing the background underneath. As a somewhat hacky solution it has been given its own toggle if issues pop up. |
| `display.vsync = true/false`          | true    | Enable/disable forced VSync |
| `display.hdpi_fix = true/false`       | true    | GrimMod rewrites some of the window handling to always render at native resolution. Since the game's UI natively scales, this should only be a positive but it can be disabled if it causes issues. |
| `logging.enabled = true/false`        | true    | Enable/disable creation of and writing to `grimmod.log` with simple logging info, mostly for the purposes of a health check. |
| `logging.debug = true/false`          | false   | Enable/disable debug logging. This outputs a lot of information per frame, useless outside of debugging/development. |

## Building

The project is currently built with Rust 1.77 Nightly on Windows.

The project needs libvpx to available in order to build. Better documentation for this is todo.
