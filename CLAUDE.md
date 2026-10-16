# CLAUDE.md

What dropkitten does and how it is used is in `README.md`. This file covers
the code.

## Layout

- `src/main.rs`: the clap CLI, `Size` (pixels or fraction) and `resolve`,
  and backend detection from `$NIRI_SOCKET` / `$SWAYSOCK`.
- `src/sway.rs`: Sway over `swayipc-async`, driven with
  `futures_lite::future::block_on`. Floats the window with a `for_window`
  rule, places it with `move position mouse` while `mouse_warping` is off,
  then closes it from the window event stream on focus loss.
- `src/niri.rs`: niri over the blocking `niri-ipc` socket. Spawns the
  terminal with a one-shot `SpawnRule` so size and position apply on the
  first frame, then closes it on focus loss.

Each backend builds its own terminal command line for kitty, alacritty and
rio, keyed on the `dropkitten` app id.

## niri-ipc pin

`niri-ipc` comes from the `kronberger-droid/niri` fork, because
`Action::Spawn { rule, .. }` does not exist upstream. The flake fetches it
by output hash (`outputHashes` in `flake.nix`); bump that hash whenever
`Cargo.lock` moves the pin.

## Commands

```sh
cargo build
cargo clippy --all-targets
nix build
```
