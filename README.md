# Deck — fast workspace & pane navigator for [herdr](https://herdr.dev)

You have 22 panes and six agents running. The question is never *"where is pane 14"* — it's
**"who needs me, and what finished while I was gone?"** herdr's built-in navigator is a flat
list that has no opinion about that. **Deck** does.

- **`tab` walks the attention queue** — every blocked agent first, then everything that
  finished unseen, across all workspaces. Open, `tab`, `tab`, done.
- **Deck opens on the answer**, not the map: if an agent is waiting on you, the cursor is
  already there and `↵` goes.
- **`r` resumes** — alt-tab back to the pane you were in before this one.
- Browsing is still there when you want it: a **workspace rail**, a **focus pane** of the
  selected workspace's tabs & panes, and **`/` search** over every pane's name, path, and agent.

```
╭──────────────────────────────────────────────────────────────────╮
│   / tab for whoever needs you · / to search 2 blocked · 2 done   │
├──────────────────────────────────────────────────────────────────┤
│   WORKSPACES            │  ESD                         ◉1 ◍1 ●1  │
│                         │                                        │
│ ▎1 ◉ esd             3  │    ▸ agents                            │
│  2 ◍ load-generator  1  │  ▌ ◉ claude: refactor auth             │
│  3 ◉ noc             2  │    ● flaky test                        │
│  4 ● infra           1  │    ◍ cargo watch                       │
├──────────────────────────────────────────────────────────────────┤
│   ~/code/esd · claude: refactor auth · claude · blocked          │
│  tab needs you   r resume   ← → column   ↑ ↓ move   / search     │
╰──────────────────────────────────────────────────────────────────╯
```

### Glass

By default the panel is opaque, so it stays readable over any wallpaper. If your terminal
is translucent (Ghostty's `background-opacity` / `background-blur-radius`, or equivalent),
turn the panel see-through by creating `~/.config/herdr/deck.toml`:

```toml
glass = true
```

`HERDR_DECK_GLASS=1` / `=0` overrides the file for one run. In glass mode Deck skips its
background fills entirely — the border, text, and the selected-row highlight stay, and
your terminal's own blur shows through everything else. On an opaque terminal this just
shows the plain terminal background, which is why it is off by default.

### Theme

Colors follow herdr's theme. **For Deck to follow your OS light/dark appearance, herdr's
config must opt into switching** — `auto_switch = false` pins whatever `name` says, which
is the usual reason a light desktop still gets a dark panel:

```toml
[theme]
auto_switch = true              # <- required; false pins `name`
dark_name  = "tokyo-night"
light_name = "tokyo-night-day"
```

With no theme configured at all, Deck follows the OS appearance on its own.

Deck draws as a **floating panel**: a rounded, opaque card sized to its content and
centred in the tab, with your terminal showing through around it. On a large screen you
get a panel, not a mostly-empty full-screen app; on a small one it fills the space rather
than wasting rows on a margin.

The top-right corner is the headline: `1 blocked · 3 done` in red when someone's waiting,
falling back to a plain pane count when nothing needs you. The active row gets a soft
full-width highlight; counts align into a column; the focused workspace's `◉ ◍ ● ✓`
rollup sits in its header; and a detail strip at the bottom shows the selected pane's
path, agent, and status. Press `/` to search across every pane — matching its **name, its cwd, and its agent**,
so `te`, `~/infra`, or `claude` all work. Matching is a plain substring over all three
fields, and results are drawn from every workspace at once:

```
╭──────────────────────────────────────────────────────────────────╮
│   / te▏                                     2 blocked · 2 done   │
├──────────────────────────────────────────────────────────────────┤
│ ▌ ● flaky test                                  esd ▸ agents     │
│   ◍ loadtest agent                load-generator ▸ lg-runner     │
│   ◉ terraform                                   noc ▸ deploy     │
│                                                                  │
│                                                                  │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│   ~/code/esd · flaky test · claude · done                        │
│         type to filter   ↑ ↓ select   ↵ switch   esc back        │
╰──────────────────────────────────────────────────────────────────╯
```

Both diagrams are the real renderer's output, not hand-drawn — `▎` marks the unfocused
column's cursor, `▌` the focused one, and the panel keeps one size whether you're
browsing or searching.

## Keys

```
 Tab    next pane that needs you — blocked first, then done  (Shift-Tab: back)
 r      resume: jump straight back to your previous pane
 ← / →  switch column: workspace rail ↔ pane list      (also h / l)
 ↑ / ↓  move within the focused column                 (also j / k)
 1–9    jump straight to a workspace
 /      search every pane by name, cwd, or agent; then type · ↑/↓ · Enter
 Enter  switch to the selected workspace (rail) or pane (list)
 Esc    close   (in search mode: back to browsing)
```

### The attention queue

`Tab` cycles panes that want you, worst first: everything **blocked** (an agent is waiting
on your input), then everything **done** (it finished and you haven't seen it). `working`
and idle panes are deliberately skipped — they don't need you. When Deck opens, the cursor
starts on the head of that queue, so the common case is `prefix+d`, `↵`.

If nothing is blocked or done, Deck opens where you left off and `Tab` does nothing.

### Resume

Deck records each pane it sends you to in `$XDG_STATE_HOME/herdr-deck/recent.json`
(default `~/.local/state/herdr-deck/recent.json`). `r` jumps to the most recent one that
isn't the pane you're currently in — an alt-tab for agents. Nothing else reads the file;
deleting it just clears the history.

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
before exiting. Colors follow **your herdr theme** — see the Theme section above for the `auto_switch`
requirement. It re-reads the
snapshot on a ~1s idle tick, so a left-open navigator keeps up with renames, new panes,
and agent-status changes without reopening — including the attention queue, so a pane that
becomes blocked while you're looking joins the `tab` cycle.

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

The same glyphs appear in the workspace rail, showing each workspace's **worst** agent
state — so a blocked workspace is distinguishable without relying on colour. The pane
you're currently sitting in is tagged `here`.

## Troubleshooting

- **`Ctrl b, d` opens the built-in list** — your key still maps to herdr's navigator.
  Make sure the `[[keys.command]]` block is in `~/.config/herdr/config.toml` and you ran
  `herdr server reload-config`.
- **Nothing opens / it flashes closed** — check the plugin's stderr:
  `herdr plugin log list --plugin deck`.
- **Requires herdr ≥ 0.7.0.**

## Not yet

**Richer preview** (recent pane output / git branch) is deliberately deferred: it turns
Deck into a dashboard — something you look at — and Deck's whole advantage is being
something you pass through in under a second. `tab`-to-next-blocked mostly dissolves the
question it would answer.

Still open: a Windows named-pipe transport (macOS/Linux only for now).

## License

MIT
