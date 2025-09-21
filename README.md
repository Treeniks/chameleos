# Chameleos

Bodged together and extremely scuffed screen annotation tool for [Hyprland](https://hypr.land/).

## [Bodge](https://youtu.be/lIFE7h3m40U?si=sMW4SCzoVEnQgSsc)

This was created because we wanted a way to draw on the screen when holding presentations and lectures without having to switch from Hyprland. Instead of using the correct tools for the job, Chameleos uses [egui](https://www.egui.rs/)+[eframe](https://docs.rs/eframe/latest/eframe/). A proper solution would've been more work than it was worth, while eframe was already familiar and kept the code short.

Since Hyprland is a Wayland compositor, ideally one would use Wayland protocols to, for example, create an overlay layer to draw in. eframe on the other hand is made for native and web applications, and uses [winit](https://docs.rs/winit/latest/winit/) under the hood, which [doesn't yet support creating Wayland layer shells](https://github.com/rust-windowing/winit/issues/2582).

As such, Chameleos requires some fiddling around with Hyprland configs and strange workarounds to get it working sensibly.

### How?

Chameleos just renders a fully transparent window over the entire screen and lets the mouse pass through it when not drawing. eframe conveniently offers a [`mouse_passthrough`](https://docs.rs/egui/latest/egui/viewport/struct.ViewportBuilder.html#method.with_mouse_passthrough) option for viewports, only it doesn't work...

Hence, we use Hyprland's [`nofocus`](https://wiki.hypr.land/Configuring/Window-Rules/#dynamic-rules) windowrule instead. But since dynamically enabling and disabling windowrules doesn't work either, we set the `nofocus` windowrule to trigger for applications with the `chameleos-passthrough` title, and then change the title appropriately in Chameleos.

We also don't automatically set Chameleos's window to cover the whole screen. Hence, we display a fully purple window on start so you can move it in place for your convenience.

## Hyprland config

```
# disable all effects
windowrule = noanim, class:^(chameleos)$
windowrule = noblur, class:^(chameleos)$
windowrule = noborder, class:^(chameleos)$
windowrule = nodim, class:^(chameleos)$
windowrule = noshadow, class:^(chameleos)$

# make mouse and keyboard passthrough work
windowrule = nofocus, title:^(chameleos-passthrough)$

# start as float
windowrule = float, class:^(chameleos)$
# pin: make window appear on all workspaces
windowrule = pin, class:^(chameleos)$

# as an alternative to manually moving the window in place
# you can also hardcode its position however you want
# windowrule = size, 1920 1080, class:^(chameleos)$
# windowrule = move, 0 0, class:^(chameleos)$
```

### Keybinds

When chameleos is in passthrough mode, we need to explicitly tell Hyprland to pass our "passthrough disable" keybind to Chameleos (making it a global hotkey). Here, as an example, we use `c` as the passthrough disable keybind in Chameleos and use `SUPER+c` to pass it to Chameleos.

```
bind = SUPER, C, pass, class:^(chameleos)$
```

## Why "Chameleos"?

![Chameleos](https://monsterhunterwiki.org/images/a/a5/MHRS-Chameleos_Render.png)

wook at his cutie widdle tongue (≧◡≦)
