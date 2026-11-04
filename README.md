# CustomMacColors

Any color as the macOS accent color, and any color as the icon tint, in every app. macOS only offers
its presets; this takes a hex code. macOS 26 on Apple silicon, Rust.

The accent changes checkboxes, radio buttons, switches, sliders, progress bars, default buttons,
selected segments, list and sidebar selection and folder icons, in AppKit and SwiftUI, in apps
already open and without touching System Settings. The icon tint is macOS's own "Tinted" icon color
(System Settings ▸ Appearance ▸ Icon & widget style), set to any color, so the Dock, Finder and
widgets follow it too.

Not covered: form controls in web pages, the Dock's own accent, and anything else drawn out of
process (`docs/accent.md` has the details).

## What it needs

It loads a small library into every app, so it needs what that takes:

- SIP's debugging restrictions off (`csrutil enable --without debug`, from Recovery).
- The `-arm64e_preview_abi` boot-arg, for arm64e apps.
- Rust through `rustup` (the toolchain is pinned in `rust-toolchain.toml`) and the Xcode command line
  tools.

## Installing

```sh
make
sudo target/dist/custommaccolors install
```

That installs a root LaunchDaemon, `io.github.eriskii.custommaccolors`, which loads the library into
apps as they start, the settings window as `/Applications/CustomMacColors.app`, and the
`custommaccolors` command. Apps already open get the color when they're relaunched, or with
`sudo custommaccolors inject <pid>`. `sudo custommaccolors uninstall` takes it all out again.

## Settings

The settings window, `custommaccolors settings`, or:

```sh
custommaccolors set accent '#FF6B00'     # or system
custommaccolors set icon-tint '#00A0A0'  # or accent
custommaccolors status
```

They're saved to `~/.config/custommaccolors/config.toml`, and reach every app at once. Run
`custommaccolors reload` after editing the file by hand. The first time, with no file yet, the colors
in `~/.config/erismenus/config.toml` are used if it's there.

The daemon never loads into the login window, the password prompt or the Dock (`SKIPPED` in
`crates/custommaccolors/src/main.rs`).

## Reading the color from another program

Your program can follow the accent without depending on this one. The settings are the state of the
Darwin notification `io.github.eriskii.custommaccolors.settings`, a `u64` read with
`notify_register_check` and `notify_get_state` (posted on each change). These bits stay as they are:

| Bits | Meaning |
|---|---|
| 63 | Valid. Without it, CustomMacColors isn't running (or hasn't published anything). |
| 24 | A custom accent is on. |
| 0 to 23 | The accent, as 0xRRGGBB. |

ErisMacShell's menus highlight in this color when it's on.

## Layout

```
crates/core             the settings, packed; no_std
crates/payload          the dylib loaded into apps; accent.rs has the how
crates/custommaccolors  the command, the daemon and the settings window
docs/accent.md          how AppKit picks the accent, and what was found by probing
tools/accenthost.m      a board of controls that follows the accent, captured to PNGs
tools/accent/           the prototype and probes behind docs/accent.md
```

`make test` runs the unit tests. `make self-test` maps the library into the command itself, as the
daemon would, without root.

```sh
make accenthost
target/dist/accenthost target/dist/libcustommaccolors.dylib /tmp/board 6
```

shows the board and captures it every 2 seconds; meanwhile `custommaccolors set accent` changes it
live. Each frame prints `controlAccentColor` as the app sees it. Two things to know when testing:
`set` also saves macOS's icon tint in your real preferences, whatever `HOME` is, so with a scratch
`HOME` write the file by hand and use `reload`; and a notification's state lasts only as long as
someone is registered for it, which here is the host (or the daemon).

## Where it came from

This code was ErisMenus' accent color feature (now ErisMacShell), split out so it can be used alone;
it starts here with a fresh history. The loading is
[ErisMacInjectLib](https://github.com/Eriskii/ErisMacInjectLib).

## License

GPL-3.0-or-later.
