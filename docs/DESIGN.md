# Deck — a card-style workspace navigator for herdr

> Design spec · 2026-07-17 · target herdr ≥ 0.7.0 (developed against 0.7.3)

## 1. Problem

herdr's built-in navigator (`Ctrl b + g`) renders every workspace → tab → pane as one
long, flat, indented tree. Two problems compound as the session grows:

1. **Navigation is linear.** Reaching a workspace near the bottom means scrolling past
   everything above it. There is no lateral movement.
2. **The workspace grouping is visually flattened.** You cannot see, at a glance, *which
   workspace needs attention* — e.g. which one has a `blocked` agent — without reading the
   whole list.

## 2. Goal

Replace the picker experience with a **horizontal deck of workspace cards**:

- **`← / →`** move between workspaces (cards).
- **`↑ / ↓`** (and `j / k`) move within the active workspace (its tabs + panes).
- **`Enter`** switches to the selected target.
- Each card is a **status dashboard**: a header rollup of agent-state counts plus a
  colored "worst-state" stripe, so a blocked/working workspace is spottable across the
  whole deck without entering it. This is the headline value, confirmed during design.

The interaction and visual language were validated with two interactive HTML mockups
(`mockup.html` = deck, `carousel.html` = alternative). **Deck** was chosen.

## 3. Why a plugin (and what that constrains)

This ships as a **standalone herdr plugin**, not a core change. herdr's plugin model fits:

- A plugin is a directory with a `herdr-plugin.toml` manifest plus an executable program.
  "The entire herdr CLI is the plugin API."
- **Honest limitation (plugin v1):** *"native non-terminal plugin UI is not part of plugin
  v1."* So this cannot literally *be* herdr's internal overlay widget. Instead it is a
  **terminal program herdr launches in a modal popup pane** that paints the deck itself.
  For an open → pick → close picker this is functionally identical: modal, same look, same
  keys, receives all input including `Esc`, closes on exit.
- **Keybinding caveat:** `Ctrl b + g` is currently the built-in navigator. We bind our
  action to `prefix+g`; the user rebinds/keeps the built-in. If plugin bindings cannot
  override a built-in prefix key, the fallback is a neighbor key (`prefix+G`). To confirm
  during implementation.

## 4. Architecture

```
┌──────────────────────────── herdr (server) ────────────────────────────┐
│  keybind  prefix+g ──▶ plugin_action "deck.open"                         │
│                              │                                           │
│                              ▼                                           │
│                     herdr plugin pane open --placement popup             │
│                              │  launches ↓                               │
└──────────────────────────────┼──────────────────────────────────────────┘
                               ▼
                    ┌───────────────────────┐        socket (HERDR_SOCKET_PATH)
                    │  herdr-deck (Rust TUI) │◀─────── session.snapshot  (bootstrap)
                    │  ratatui + crossterm   │◀─────── events.subscribe  (live, opt.)
                    │                        │───────▶ pane.focus / tab.focus /
                    │  draws the card deck   │         workspace.focus  (on Enter)
                    └───────────────────────┘
```

### Components

1. **`herdr-plugin.toml`** — manifest declaring: the `deck.open` action, a `picker` popup
   pane entrypoint, and the `prefix+g` keybinding.
2. **`herdr-deck`** — a Rust binary (ratatui + crossterm) that is the popup command. It:
   - connects to the herdr socket via `HERDR_SOCKET_PATH`,
   - fetches `session.snapshot`, builds the model, renders the deck,
   - handles keys, and on `Enter` issues the focus request, then exits.
3. **Socket client module** — minimal JSON-RPC over the Unix socket / Windows named pipe
   (mirrors herdr's own `interprocess` usage). Methods used: `session.snapshot`,
   `pane.focus`, `tab.focus`, `workspace.focus`, and (optional) `events.subscribe`.

### Why speak the socket directly (not shell out)

`pane.focus` (absolute, by pane id) is a socket method; the CLI only exposes directional
pane focus. Speaking JSON-RPC over `HERDR_SOCKET_PATH` gives us snapshot + absolute focus +
live events in one transport, matching herdr's own IPC. `HERDR_BIN_PATH` shell-outs remain
a portable fallback for `workspace focus` / `tab focus`.

## 5. Data model (verified against live `session.snapshot`)

`result.snapshot` contains `workspaces[]`, `tabs[]`, `panes[]`, `agents[]`, `layouts[]`,
and `focused_workspace_id / focused_tab_id / focused_pane_id`.

| Record | Fields we use |
|---|---|
| workspace | `workspace_id`, `label`, `number`, `agent_status`, `active_tab_id`, `focused`, `pane_count`, `tab_count` |
| tab | `tab_id`, `workspace_id`, `label`, `number`, `agent_status`, `focused`, `pane_count` |
| pane | `pane_id`, `tab_id`, `workspace_id`, `agent_status`, `cwd`, `focused`, `terminal_id` |

`agent_status ∈ { blocked, working, done, idle, unknown }` — the rollup counts are computed
by grouping panes by `workspace_id` and counting per status. No new data is needed.

Internal model (built from the snapshot):

```
Workspace { id, label, tabs: [Tab], rollup: Counts, worst: Status, is_current, active_tab_id }
Tab       { id, label, panes: [Pane], pane_count }
Pane      { id, label, status: Status, meta, is_current }
Row       = Workspace-row | Tab-row | Pane-row   // ↑/↓-selectable list inside a card
```

Row 0 of every card is the **workspace-row** (the card header, selectable — `Enter` on it
focuses the workspace's active tab), followed by each tab-row and its pane-rows. This
mirrors the built-in navigator, where the workspace itself is a selectable target.

## 6. UX spec (carried over from the approved deck mockup)

**Palette:** Catppuccin Mocha (herdr default) — `base #1e1e2e`, `text #cdd6f4`,
`red #f38ba8`, `yellow #f9e2af`, `teal #94e2d5`, `green #a6e3a1`, `overlay0 #6c7086`,
accent (selection) `peach #fab387`. Read the active theme from herdr where feasible; fall
back to Mocha. (Theme sourcing is a v1.1 nicety — see Open Questions.)

**Status glyphs** (identical to herdr's `agent_icon`):

| status | glyph | color |
|---|---|---|
| blocked | `◉` | red |
| working | spinner `⠋…` | yellow |
| done | `●` | teal |
| idle | `✓` | green |
| unknown (shell) | `○` | overlay0 |

**Card anatomy** (top → bottom):
- Header: `◆` current-marker + workspace label (left), pane count (right).
- Left status stripe (1 col): colored by the worst status in the workspace
  (blocked ▸ working ▸ done ▸ idle ▸ unknown).
- Rollup line: `◉n ◍n ●n ✓n`, each dim when zero.
- Body: flattened tab + pane rows; `↑/↓` selectable; vertical scroll when taller than card.

**Deck behavior:**
- Cards laid horizontally at a fixed width; active card highlighted (accent border + row
  highlight), inactive cards dimmed but fully rendered.
- Horizontal scroll offset keeps the active card in view; `‹`/`›` edge hints when more
  cards exist off-screen.

**Preserved from the built-in navigator:** `/` search, `b/w/i/d/a` state filters (dim/hide
non-matching panes; hide workspaces with no matches under an active filter), a detail line,
and the footer hint bar.

## 7. Navigation state machine

```
state = { active: usize,               // active workspace index
          sel:    Vec<usize>,          // remembered selected row per workspace
          hscroll: usize,              // deck horizontal offset
          query: String, filter: Option<Status> }

→ / l      active = min(active+1, n-1);  clamp sel[active];  reveal active
← / h      active = max(active-1, 0);    clamp sel[active];  reveal active
↓ / j      sel[active] = min(sel[active]+1, rows(active)-1)   // clamped WITHIN workspace
↑ / k      sel[active] = max(sel[active]-1, 0)
Enter      focus(target of rows(active)[sel[active]]); exit
/          enter search; b/w/i/d/a toggle filters; Esc clears filter/query then quits
```

`Enter` target → request:
- Workspace row → `workspace.focus { workspace_id }`
- Tab row → `tab.focus { tab_id }`
- Pane row → `pane.focus { pane_id }`

## 8. Edge cases

- **1 workspace** → single card; `←/→` no-op.
- **Many workspaces** → horizontal scroll; active stays centered-ish.
- **Workspace with many panes** → card body scrolls vertically.
- **Terminal too narrow** for a full card → render a single full-width card (still usable);
  never crash.
- **Empty snapshot / socket error** → show a one-line error in the popup and exit cleanly.
- **Snapshot is a point-in-time bootstrap** → v1 renders it as-is; live refresh is optional
  (§10).

## 9. Distribution

- Repo: `herdr-plugin-deck` (GitHub topic `herdr-plugin` for marketplace listing).
- `min_herdr_version = "0.7.0"`.
- Plugin `id = "deck"` (author may namespace, e.g. `raghu.deck`); binary `herdr-deck`.
- Manifest declares `[[actions]] id="open"` (qualified `deck.open`), `[[panes]] id="picker"
  (placement="popup", width="90%", height="85%")`, and `[[keys.command]] key="prefix+g"`.
- Build: `cargo build --release`; document `cargo` as the required toolchain. Consider
  prebuilt binaries per-platform as a later convenience.
- Local dev via `herdr plugin link .`; install via `herdr plugin install <owner>/herdr-plugin-deck`.

## 10. Open questions / risks (resolve during implementation)

1. **Keybind precedence** — can a plugin `prefix+g` override the built-in navigator, or must
   the user rebind the built-in? Fallback: `prefix+G`.
2. **Live updates** — v1 renders the bootstrap snapshot. Should the popup `events.subscribe`
   and repaint on `pane.agent_status_changed`? Nice-to-have; adds a socket read loop.
3. **Theme sourcing** — read the user's active herdr theme, or ship Mocha + a config knob?
   v1: Mocha default, optional `HERDR_PLUGIN_CONFIG_DIR/config.toml` override.
4. **`done` vs `idle`** — snapshot pre-collapses the `seen` flag into `done`/`idle`; confirm
   this matches the built-in navigator's meaning (it should — same enum).

## 11. Out of scope (YAGNI for v1)

Carousel / focus-zoom layout, mouse drag, reordering workspaces, creating/closing
workspaces from the picker, animations beyond terminal-native repaint.

## 12. Testing approach (TDD)

- **Model/pure logic** (unit tests, no herdr needed): snapshot JSON → internal model;
  rollup counting; worst-status; row flattening; navigation reducer (`←/→/↑/↓` transitions
  and clamping); filter/search predicates. These are the bulk and are fully testable with
  fixture JSON captured from `herdr api snapshot`.
- **Socket client**: test request encoding + response decoding against captured fixtures;
  a thin integration test against a live socket behind a feature flag.
- **Render**: ratatui buffer snapshot tests for card layout at a few widths (incl. the
  narrow-fallback path).
- **Manual acceptance**: `herdr plugin link .`, bind the key, drive the popup in a real
  session with multiple workspaces and a blocked agent.
```
