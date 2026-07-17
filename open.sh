#!/bin/sh
# Opens the deck navigator as a zoomed overlay over the active pane.
# herdr 0.7.x has no "popup" placement; "overlay" is the modal-style option —
# a temporary full-screen pane that restores your previous view when it closes.
# Uses HERDR_BIN_PATH (injected by herdr) so it stays portable across sockets/pipes.
exec "${HERDR_BIN_PATH:-herdr}" plugin pane open --plugin deck --entrypoint picker --placement overlay
