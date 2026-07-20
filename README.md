# Deck — fast workspace & pane navigator for [herdr](https://herdr.dev)

herdr's built-in navigator is a single flat list of every workspace → tab → pane. With
5+ workspaces and lots of panes that means a lot of scrolling. **Deck** replaces it with a
navigator built to reach any pane fast **without scrolling**:

- a **rail** of all your workspaces (never scrolls — jump to any with `1`–`9`),
- a **focus pane** showing only the selected workspace's tabs & panes (short list),
- **`/` search** across every pane in every workspace, with live results.

```
  / press / to search · 1–9 to jump                          22 panes
 ─────────────────────────────────────────────────────────────────────
  WORKSPACES              │  LOAD-GENERATOR              ◉0 ◍1 ●0 ✓1
                          │
 ▌1 ● esd             5   │    ▸ lg-runner
  2 ● load-generator  2   │  ▌ ✓ loadtest agent
  3 ● noc             2   │    ▸ deploy
  4 ● infra           3   │      ◍ terraform
 ─────────────────────────────────────────────────────────────────────
  ~/code/load-generator · loadtest agent · claude · idle
  ← → column   ↑ ↓ move   1–9 jump   / search   ↵ switch   esc close
```

The active row gets a soft full-width highlight; counts align into a column; the
focused workspace's `◉ ◍ ● ✓` rollup sits in its header; and a detail strip at the
bottom shows the selected pane's path, agent, and status. Press `/` to search across
every pane:

```
  / tf                                                        22 panes
 ─────────────────────────────────────────────────────────────────────
 ▌◍ terraform              infra ▸ deploy
  ◉ tf-plan                infra ▸ tf-plan
 ─────────────────────────────────────────────────────────────────────
  ~/infra · terraform · idle
  type to filter   ↑ ↓ select   ↵ switch   esc back
```

## Keys

```
 ← / →  switch column: workspace rail ↔ pane list      (also h / l)
 ↑ / ↓  move within the focused column                 (also j / k)
 1–9    jump straight to a workspace
 /      search every pane across all workspaces; then type · ↑/↓ · Enter
 Enter  switch to the selected workspace (rail) or pane (list)
 Esc    close   (in search mode: back to browsing)
```

Navigation is Miller-columns style: `←/→` moves the cursor between the two columns, and
`↑/↓` always moves within whichever column has it. The focused column shows a bright
cursor; the other a faint one.

The rail shows every workspace with a status dot colored by its **worst** agent state, so
a workspace with a blocked agent (`●` red) stands out at a glance.

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
key = "prefix+d"          # Ctrl b, then d
type = "plugin_action"
command = "deck.open"
description = "workspace navigator"
```

`prefix+g` is herdr's built-in navigator and takes precedence, so pick a free key —
herdr's defaults already claim `b c e f g h j k l n o p q r s v w x y z ? tab`, so
`prefix+d` is free. Change `key` to taste and reload.

### Try it without a keybinding

```bash
herdr plugin pane open --plugin deck --entrypoint picker --placement tab --focus
```

## How it works

herdr launches the `herdr-deck` binary in its **own throwaway tab** (`--placement tab`),
which closes when you make a choice. The binary reads `session.snapshot` over herdr's
socket (`HERDR_SOCKET_PATH`, newline-delimited JSON), renders with
[ratatui](https://ratatui.rs), and on `Enter` issues `pane.focus` (or `workspace.focus`)
before exiting. Colors follow **your herdr theme** — it reads `~/.config/herdr/config.toml`
and matches your active light/dark theme, falling back to Catppuccin. It re-reads the
snapshot on a ~1s idle tick, so a left-open navigator keeps up with renames, new panes,
and agent-status changes without reopening.

> It deliberately uses `tab` placement, **not** `overlay`. Overlay injects a pane into
> your active tab and zooms it, and its teardown can leave your tab zoomed and your splits
> scrambled. A throwaway tab never touches any existing tab's split layout.

## Status glyphs

| state | glyph | meaning |
|---|---|---|
| blocked | `◉` red | agent is waiting on you |
| working | `◍` yellow | agent is running |
| done | `●` teal | finished, unseen |
| idle | `✓` green | idle / seen |
| unknown | `○` grey | plain shell |

## Troubleshooting

- **`Ctrl b, d` opens the built-in list** — your key still maps to herdr's navigator.
  Make sure the `[[keys.command]]` block is in `~/.config/herdr/config.toml` and you ran
  `herdr server reload-config`.
- **Nothing opens / it flashes closed** — check the plugin's stderr:
  `herdr plugin log list --plugin deck`.
- **Requires herdr ≥ 0.7.0.**

## Not yet

Richer preview (recent pane output / git branch — the detail strip already shows cwd,
agent, and status), and a Windows named-pipe transport (macOS/Linux only for now).

## License

MIT
