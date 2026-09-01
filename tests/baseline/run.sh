#!/usr/bin/env bash
# Phase 0 behavior baseline (SPECS.md §4 Phase 0).
#
# Runs every arioch CLI command against a fixed scratch tree and saves
# stdout/stderr/exit-code per command into out/. Re-run after a refactor
# and diff out/ to prove the CLI is byte-identical (SPECS.md §5).
#
# Determinism notes:
# - .scratch/ is a fixed path (not mktemp) so paths printed in outputs are stable.
# - XDG_CONFIG_HOME points at the fixture, because Config::load() reads the
#   static XDG/HOME config dir and does NOT honor --config (current behavior;
#   must be preserved). --config is still honored by the registry (index.toml).

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="$ROOT/target/debug/arioch"
BASE="$(cd "$(dirname "$0")" && pwd)"
SCRATCH="$BASE/.scratch"
XDGFIX="$SCRATCH/xdg"
OUT="$BASE/out"

if [ ! -x "$BIN" ]; then
  echo "build first: cargo build" >&2
  exit 1
fi

rm -rf "$SCRATCH" "$OUT"
mkdir -p "$OUT" \
  "$SCRATCH/files/.ssh" "$SCRATCH/files/certs" "$SCRATCH/files/creds" "$SCRATCH/files/app" \
  "$SCRATCH/scan/ssh" "$SCRATCH/scan/certs" "$SCRATCH/scan/app" \
  "$SCRATCH/scan/deep/a/b/c/d" "$SCRATCH/scan/ignored" \
  "$XDGFIX/arioch"

# ── fixture files to register ────────────────────────────────────────────────
printf 'Host prod\n  HostName prod.example.com\n  User deploy\n  Port 2222\n' > "$SCRATCH/files/.ssh/config"
printf 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5IGZpeHR1cmUK\n' > "$SCRATCH/files/.ssh/id_ed25519.pub"
: > "$SCRATCH/files/.ssh/id_ed25519"
printf -- '-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----\n' > "$SCRATCH/files/certs/server.pem"
printf 'token = "fixture-token"\n' > "$SCRATCH/files/creds/tokens.json"
printf 'SECRET=fixture\n' > "$SCRATCH/files/app/.env"

# ── fixture scan tree ────────────────────────────────────────────────────────
: > "$SCRATCH/scan/ssh/id_rsa"
: > "$SCRATCH/scan/ssh/config"
: > "$SCRATCH/scan/certs/server.pem"
printf 'x\n' > "$SCRATCH/scan/app/credentials.toml"
printf 'x\n' > "$SCRATCH/scan/app/.env"
printf 'x\n' > "$SCRATCH/scan/deep/a/b/c/key.key"      # depth 3 -> included
printf 'x\n' > "$SCRATCH/scan/deep/a/b/c/d/leaf.key"   # depth 4 -> excluded
printf 'x\n' > "$SCRATCH/scan/ignored/secret.txt"      # excluded via exclude_paths
: > "$SCRATCH/scan/single.pem"                         # file passed directly in scan_paths

# ── fixture config.toml (read by Config::load() via XDG_CONFIG_HOME) ─────────
cat > "$XDGFIX/arioch/config.toml" <<EOF
# arioch config (Phase 0 fixture)
scan_paths = ["$SCRATCH/scan/ssh", "$SCRATCH/scan/certs", "$SCRATCH/scan/app", "$SCRATCH/scan/deep", "$SCRATCH/scan/single.pem"]
exclude_paths = ["$SCRATCH/scan/ignored"]
scan_patterns = ["id_*", "*.pub", "config", "*.pem", "*.key", "credentials", "*.toml", ".env*"]
scan_depth = 3
max_file_size = 1048576
refresh_interval = 2
EOF

export XDG_CONFIG_HOME="$XDGFIX"

STEP=0
run() { # run <name> <args...>
  local name="$1"; shift
  STEP=$((STEP + 1))
  local idx
  idx=$(printf '%02d' "$STEP")
  "$BIN" --config "$SCRATCH/cfg" "$@" > "$OUT/$idx-$name.stdout" 2> "$OUT/$idx-$name.stderr"
  echo $? > "$OUT/$idx-$name.exit"
}

# 1. init a fresh config/index at the scratch dir
run init init "$SCRATCH/cfg"

# 2. init on an existing dir -> error
run init-existing init "$SCRATCH/cfg"

# 3. add without category -> exercises main.rs guess_category branches
run add-no-cat add "$SCRATCH/files/.ssh/config"
run add-token-file add "$SCRATCH/files/creds/tokens.json"
run add-env-file add "$SCRATCH/files/app/.env"

# 4. add with all fields (plain + json output)
run add-full add "$SCRATCH/files/certs/server.pem" --category certs --tags "tls,web" --description "Server certificate" --alias srv
run add-full-json add --json "$SCRATCH/files/.ssh/id_ed25519" --category ssh-keys

# 5. add nonexistent -> error
run add-missing add "$SCRATCH/files/nope.pem"

# 6. list (plain + json)
run list list
run list-json list --json
# 7. export (before tag/remove, so import exercises merge/replace semantics)
run export export -o "$SCRATCH/export.json"
cat "$SCRATCH/export.json" > "$OUT/07-export-file.json"

# 8. map (plain + json)
run map map
run map-json map --json

# 9. tag (plain + json), tag missing -> error
run tag tag "$SCRATCH/files/.ssh/config" bastion
run tag-json tag --json "$SCRATCH/files/.ssh/config" bastion
run tag-missing tag "$SCRATCH/files/ghost" x

# 10. remove (plain + json), remove again -> error
run remove remove "$SCRATCH/files/app/.env"
run remove-json remove --json "$SCRATCH/files/creds/tokens.json"
run remove-missing remove "$SCRATCH/files/ghost"

# 11. scan (plain + json)
run scan scan
run scan-json scan --json

# 12. import (merge re-adds the removed entries), then --replace
run import import "$SCRATCH/export.json"
run import-replace import --replace "$SCRATCH/export.json"

# 13. final state
run list-final list
run list-final-json list --json

# manifest of what was run
{
  echo "bin: $BIN"
  echo "scratch: $SCRATCH"
  echo "xdg_config_home: $XDGFIX"
  for f in "$OUT"/*.stdout; do
    name="$(basename "$f" .stdout)"
    echo "$name exit=$(cat "$OUT/$name.exit")"
  done
} > "$OUT/manifest.txt"

echo "baseline written to $OUT ($(ls "$OUT" | wc -l) files)"
