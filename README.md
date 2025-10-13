# Chameleos

Wayland screen annotation tool, tested for [niri](https://yalter.github.io/niri/) and [Hyprland](https://hypr.land/).

Originally [bodged together](https://github.com/Treeniks/chameleos-egui) with [eframe](https://docs.rs/eframe/latest/eframe/) for a lecture, this repository holds a complete low-level rewrite, utilizing wayland's layer shell protocol with [wayland-client](https://crates.io/crates/wayland-client) directly, path tessellation with [lyon](https://crates.io/crates/lyon) and GPU rendering with [wgpu](https://wgpu.rs/).

> [!NOTE]
> Project status: Very usable, if still barebones.

## Usage

First, install the helper utility `chamel`:
```
git clone git@github.com:Treeniks/chameleos.git
cd chameleos
cargo install --path ./chamel
```

`chamel` is used to send commands to `chameleos` while it is running. `chameleos` itself has no keyboard input functionality, all keybinds (for example to toggle input) must be handled from the compositor and `chamel`.

To run`chameleos` itself:
```
cargo run --release
```
This will create a layer shell overlay over your entire current screen in which you can draw. To toggle input, run `chamel toggle`, after which you can draw with the left mouse button or with a pen on a graphic tablet.

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

The color can be given in whatever formats the [csscolorparser](https://crates.io/crates/csscolorparser) crate supports. The color can also include opacity, although due to a technical limitation, `chameleos` uses `wgpu::CompositeAlphaMode::PreMultiplied` which makes opacity behave a bit weird.

### Eraser

The only eraser type currently supported is a stroke eraser. It is mapped to the right mouse button as well as pen button 1 for graphic tablets (Linux Artist Mode in [OpenTabletDriver](https://opentabletdriver.net/)). Remapping this is currently not supported. To improve performance, `chameleos` may sometimes split lines into multiple segments if they get too long, in which case only one of these segments will get erased instead of the entire line.

## Why "Chameleos"?

![Chameleos](https://monsterhunterwiki.org/images/a/a5/MHRS-Chameleos_Render.png)

wook at his cute widdle tongue (≧◡≦)
