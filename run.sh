#!/bin/sh
# Launches the deck binary by absolute path. herdr's pane launcher resolves the
# pane command via PATH (not the plugin cwd), so we can't name the relative binary
# directly. HERDR_PLUGIN_ROOT is injected by herdr and points at the plugin dir.
exec "${HERDR_PLUGIN_ROOT:-.}/target/release/herdr-deck"
