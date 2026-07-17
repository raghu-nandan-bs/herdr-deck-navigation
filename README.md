# Deck — card-style workspace navigator for [herdr](https://herdr.dev)

Replaces herdr's flat workspace/tab/pane list with a horizontal **deck of workspace
cards**. Each card is a status dashboard: a rollup of agent-state counts and a colored
"worst-status" stripe, so a workspace with a **blocked** agent is spottable across the
whole deck without entering it.

```
      ╭─ api ◆ ──────────╮   ╭─ web ────────────╮   ╭─ infra ──────────╮
      ┃ ◉0 ◍2 ●1 ✓1      │   ┃ ◉1 ◍2 ●0 ✓1      │   ┃ ◉0 ◍0 ●0 ✓1      │
      ┃  workspace       │   ┃  workspace       │   ┃  workspace       │
      ┃ ▸ server         │   ┃ ▸ dev            │   ┃ ▸ shell          │
      ┃   ✓ pane 1       │   ┃   ✓ pane 2       │   ┃   ✓ pane 4       │
      ┃ ▸ build          │   ┃ ▸ worker         │   ┃ ▸ editor         │
      ┃   ◍ pane 2       │   ┃   ◍ pane 3       │   ┃   ○ nvim         │
      ┃ ▸ test           │   ┃ ▸ debug          │   ╰──────────────────╯
      ┃  «● pane 8»      │   ┃   ◉ pane 7       │
      ┃   ◍ pane 9       │   ┃   ◍ pane 8       │
      ╰──────────────────╯   ╰──────────────────╯
        ▲ active card          ▲ ┃ stripe is red: a blocked agent lives here
        «…» selected row

      enter switch   ← → workspace   ↑ ↓ pane   b/w/i/d filter   esc close
```

```
  ← / →   move between workspaces (cards)
  ↑ / ↓   move within the active workspace (its tabs + panes)   (also j / k)
  Enter   switch to the selected workspace / tab / pane
  b w i d  filter by agent state (blocked / working / idle / done)
  Esc     close
```

## Install

Requires the Rust toolchain (`cargo`) — herdr builds the plugin from source on install.

### From GitHub (recommended)

```bash
herdr plugin install raghu-nandan-bs/herdr-deck-navigation
herdr plugin list                       # "deck" should appear, enabled
```

`herdr plugin install` clones the repo, runs `cargo build --release`, and registers the
plugin. To update later, re-run the same command.

### From source (local dev)

```bash
git clone https://github.com/raghu-nandan-bs/herdr-deck-navigation
cd herdr-deck-navigation
cargo build --release
herdr plugin link "$PWD"                 # link the working dir instead of installing
```

### Bind a key

herdr reads keybindings from **`~/.config/herdr/config.toml`**, not from the plugin
manifest. Add this block, then reload with `herdr server reload-config`:

```toml
[[keys.command]]
key = "prefix+d"          # Ctrl b, then d  ("d" for deck)
type = "plugin_action"
command = "deck.open"
description = "card navigator (deck)"
```

`prefix+g` is herdr's built-in navigator and takes precedence, so pick a free key —
herdr's defaults already claim `b c e f g h j k l n o p q r s v w x y z ? tab`, so
`prefix+d` is free. Change `key` to taste and reload.

### Try it without a keybinding

```bash
herdr plugin pane open --plugin deck --entrypoint picker --placement overlay
```

## How it works

herdr launches the `herdr-deck` binary in an **overlay pane** (a temporary full-screen
pane that restores your previous view on close). The binary reads `session.snapshot` over
herdr's socket (`HERDR_SOCKET_PATH`, newline-delimited JSON), renders the deck with
[ratatui](https://ratatui.rs), and on `Enter` issues `workspace.focus` / `tab.focus` /
`pane.focus` before exiting.

> Plugin v1 does not allow native in-app UI, so Deck is a self-drawn terminal pane rather
> than a replacement for herdr's built-in overlay. herdr 0.7.x has no `popup` placement,
> so it uses `overlay`. Functionally it's the same: full-screen, same keys, closes on exit.

## Status glyphs

| state | glyph | meaning |
|---|---|---|
| blocked | `◉` red | agent is waiting on you |
| working | `◍` yellow | agent is running |
| done | `●` teal | finished, unseen |
| idle | `✓` green | idle / seen |
| unknown | `○` grey | plain shell |

The card header shows the per-state counts (`◉n ◍n ●n ✓n`); the left stripe is colored by
the **worst** status in that workspace (blocked ▸ working ▸ done ▸ idle).

## Troubleshooting

- **`Ctrl b, d` opens the built-in list** — your key still maps to herdr's navigator. Make
  sure the `[[keys.command]]` block is in `~/.config/herdr/config.toml` and you ran
  `herdr server reload-config`.
- **Nothing opens / it flashes closed** — check the plugin's stderr:
  `herdr plugin log list --plugin deck`.
- **Requires herdr ≥ 0.7.0.**

## Not yet (v1 scope)

Free-text search box (state filters ship in v1), live event-stream refresh (renders a
point-in-time snapshot), Windows named-pipe transport, and per-pane titles (shows `pane N`).

## License

MIT
