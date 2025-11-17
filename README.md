# Chameleos

[![License](https://img.shields.io/github/license/Treeniks/chameleos)](https://github.com/Treeniks/chameleos/blob/master/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/Treeniks/chameleos/ci.yml?label=ci)
](https://github.com/Treeniks/chameleos/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Treeniks/chameleos
)](https://github.com/Treeniks/chameleos/releases)

Wayland screen annotation tool, tested for [niri](https://yalter.github.io/niri/) and [Hyprland](https://hypr.land/).

https://github.com/user-attachments/assets/347d9f77-437f-4793-9df3-1696dd4df926

Originally [bodged together](https://github.com/Treeniks/chameleos-egui) with [eframe](https://docs.rs/eframe/latest/eframe/) for a lecture, this repository holds a complete low-level rewrite, utilizing wayland's layer shell protocol with [wayland-client](https://crates.io/crates/wayland-client) directly, path tessellation with [lyon](https://crates.io/crates/lyon) and GPU rendering with [wgpu](https://wgpu.rs/).

## Install

The chameleos packages will install both `chameleos` and `chamel`.

### AUR (Arch Linux)

[![chameleos](https://img.shields.io/aur/version/chameleos?logo=Arch%20Linux&label=chameleos
)](https://aur.archlinux.org/packages/chameleos)
[![chameleos-bin](https://img.shields.io/aur/version/chameleos-bin?logo=Arch%20Linux&label=chameleos-bin
)](https://aur.archlinux.org/packages/chameleos-bin)
[![chameleos-git](https://img.shields.io/aur/version/chameleos-git?logo=Arch%20Linux&label=chameleos-git
)](https://aur.archlinux.org/packages/chameleos-git)

E.g. using your favorite AUR helper:
```sh
paru -S chameleos
```

### Cargo

[![Crates.io](https://img.shields.io/crates/v/chameleos)](https://crates.io/crates/chameleos)

```sh
cargo install --locked chameleos
```

### Build From Source

```sh
git clone https://github.com/Treeniks/chameleos
cd chameleos
cargo build --locked --release
```

## Usage

`chamel` is a helper utility used to send commands to `chameleos` while it is running. `chameleos` itself has no keyboard input functionality, all keybinds (for example to toggle input) must be handled from the compositor and `chamel`.

To start `chameleos`:
```sh
chameleos &
```
This will create a layer shell overlay over your entire current screen in which you can draw. There is currently no way to switch display after start. To toggle input, run `chamel toggle`, after which you can draw with the left mouse button or with a pen on a graphic tablet.

Example keybind configuration in niri:
```kdl
F1 { spawn "chamel" "toggle"; }
F2 { spawn "chamel" "undo"; }
F3 { spawn "chamel" "clear"; }
F4 { spawn "chamel" "exit"; }
```
To see a list of commands, run `chamel help`.

### Stroke Color and Width

The stroke width can be set
- on startup with `chameleos --stroke-width 16` (default is 8)
- on the fly with `chamel stroke-width 16`

The stroke color can be set
- on startup with `chameleos --stroke-color "#00BFFF"` (default is `#FF0000`)
- on the fly with `chamel stroke-color "#00BFFF"`

The color can be given in whatever formats the [csscolorparser](https://crates.io/crates/csscolorparser) crate supports. The color can also include opacity, so you could make a highlighter pen. Multiple pens aren't explicitly supported, but the same can be achieved with respective stroke-color and stroke-width keybinds.

### Eraser

The only eraser type currently supported is a stroke eraser. It is mapped to the right mouse button as well as pen button 1 for graphic tablets (Linux Artist Mode in [OpenTabletDriver](https://opentabletdriver.net/)). Remapping this is currently not supported. To improve performance, `chameleos` may sometimes split lines into multiple segments if they get too long, in which case only one of these segments will get erased instead of the entire line.

## Logging

We use [`env_logger`](https://docs.rs/env_logger/latest/env_logger/) for logging. Chameleos specific logging targets are:

- `chameleos::general`
- `chameleos::socket`
- `chameleos::wayland`
- `chameleos::render`

## Why "Chameleos"?

![Chameleos](https://monsterhunterwiki.org/images/a/a5/MHRS-Chameleos_Render.png)

wook at his cute widdle tongue (≧◡≦)
