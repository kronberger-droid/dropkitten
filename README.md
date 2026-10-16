# dropkitten

Dropdown terminals for Sway and niri. dropkitten opens a floating terminal
under the cursor, runs a command in it, and closes it once it loses focus.

```sh
dropkitten -t kitty                            # a dropdown shell
dropkitten -t kitty -W 0.35 -H 0.45 -- wiremix # a TUI as a popup
dropkitten -t alacritty -W 800 -H 600 -c -- btop
```

- **Sway and niri.** The backend is picked from `$SWAYSOCK` or
  `$NIRI_SOCKET`.
- **Gone on focus loss.** Click elsewhere and the window closes, so a
  status-bar button can toggle a TUI like a menu.
- **Sizes in pixels or fractions.** `-W 800` is 800 px, `-W 0.35` is 35% of
  the focused output. The default is 30% by 40%.
- **Placement.** Centered horizontally on the cursor and pinned to the top
  edge; `-c` centers it vertically, `-x` and `-y` shift it.
- **kitty, alacritty or rio**, chosen with `-t`.

## Install

```sh
nix run github:kronberger-droid/dropkitten -- --help
```

Or `cargo build --release`. The niri backend opens the window through a
one-shot spawn rule, which upstream niri does not have yet: it needs the
[kronberger-droid/niri](https://github.com/kronberger-droid/niri) fork that
`niri-ipc` is pinned to. Sway works with any release.

## License

[MIT](LICENSE)
