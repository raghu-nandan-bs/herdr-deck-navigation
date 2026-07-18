#!/bin/sh
# Opens the navigator in its own throwaway tab. IMPORTANT: use "tab" placement,
# NOT "overlay" — overlay injects a pane into your *active* tab and zooms it, and
# its teardown can leave your tab zoomed / splits scrambled. "tab" placement calls
# herdr's create_tab and never touches existing tabs' split layouts; the tab closes
# when the picker exits. HERDR_BIN_PATH is injected by herdr.
exec "${HERDR_BIN_PATH:-herdr}" plugin pane open --plugin deck --entrypoint picker \
  --placement tab --focus
