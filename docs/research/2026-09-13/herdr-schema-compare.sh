#!/usr/bin/env bash
set -u
SP="/mnt/c/Users/<user>/AppData/Local/Temp/claude/D--projects-ai-cockpit/c29a7fad-0a4d-474f-8197-a4e88ed08312/scratchpad/herdr-research"
~/.local/bin/herdr api schema --json > "$SP/schema-wsl.json" 2>"$SP/schema-wsl.stderr" || { echo "schema export failed"; cat "$SP/schema-wsl.stderr"; exit 1; }
W="$SP/schema.json"; L="$SP/schema-wsl.json"
echo "== protocol =="; echo "windows: $(jq -r '.protocol' "$W")  wsl: $(jq -r '.protocol' "$L")  wsl herdr: $(~/.local/bin/herdr --version)"
ext_defs() { jq -r --arg k "$2" '[.. | objects | to_entries[] | select(.key==$k) | .value.enum[]?] | unique[]' "$1"; }
ext_props() { jq -r --arg k "$2" '[.. | objects | to_entries[] | select(.key==$k) | .value.properties? // {} | keys[]] | unique[]' "$1"; }
jq -r '.schemas.request.oneOf[].properties.method.const' "$W" | sort > "$SP/win-methods.txt"
jq -r '.schemas.request.oneOf[].properties.method.const' "$L" | sort > "$SP/wsl-methods.txt"
echo "== methods: win=$(wc -l < "$SP/win-methods.txt") wsl=$(wc -l < "$SP/wsl-methods.txt") =="
echo "-- only in windows:"; comm -23 "$SP/win-methods.txt" "$SP/wsl-methods.txt" | tr '\n' ' '; echo
echo "-- only in wsl:"; comm -13 "$SP/win-methods.txt" "$SP/wsl-methods.txt" | tr '\n' ' '; echo
for k in EventKind SubscriptionEventKind AgentStatus ReadSource; do
  echo "== enum $k =="
  ext_defs "$W" "$k" | sort > "$SP/win-$k.txt"; ext_defs "$L" "$k" | sort > "$SP/wsl-$k.txt"
  echo "win=$(wc -l < "$SP/win-$k.txt") wsl=$(wc -l < "$SP/wsl-$k.txt")"
  echo "-- only win: $(comm -23 "$SP/win-$k.txt" "$SP/wsl-$k.txt" | tr '\n' ' ')"
  echo "-- only wsl: $(comm -13 "$SP/win-$k.txt" "$SP/wsl-$k.txt" | tr '\n' ' ')"
done
for k in SessionSnapshot WorkspaceInfo PaneInfo AgentInfo PaneReadResult PaneReadParams EventsSubscribeParams; do
  echo "== props $k =="
  ext_props "$W" "$k" | sort > "$SP/win-$k.txt"; ext_props "$L" "$k" | sort > "$SP/wsl-$k.txt"
  echo "win=$(wc -l < "$SP/win-$k.txt") wsl=$(wc -l < "$SP/wsl-$k.txt")"
  echo "-- only win: $(comm -23 "$SP/win-$k.txt" "$SP/wsl-$k.txt" | tr '\n' ' ')"
  echo "-- only wsl: $(comm -13 "$SP/win-$k.txt" "$SP/wsl-$k.txt" | tr '\n' ' ')"
done
echo "== subscription kinds (win) =="; jq -r '[.. | objects | to_entries[] | select(.key=="Subscription") | .value.oneOf[]? | (.properties.type.const? // .const? // "?")] | unique | join(" ")' "$W"
echo "== subscription kinds (wsl) =="; jq -r '[.. | objects | to_entries[] | select(.key=="Subscription") | .value.oneOf[]? | (.properties.type.const? // .const? // "?")] | unique | join(" ")' "$L"
