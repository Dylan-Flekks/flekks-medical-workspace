#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
home=${HOME:?HOME must be set to launch the medical workspace}
sqlite_home=${FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME:-"$home/.codex/flekks-medical-synthetic"}
codex_home=${CODEX_HOME:-"$home/.codex"}

case "$sqlite_home" in
    /*) ;;
    *)
        echo "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME must be an absolute path." >&2
        exit 2
        ;;
esac

case "$codex_home" in
    /*) ;;
    *)
        echo "CODEX_HOME must be an absolute path for the medical workspace." >&2
        exit 2
        ;;
esac

umask 077
mkdir -p "$sqlite_home"

sqlite_home=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$sqlite_home")
codex_home=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$codex_home")
if [ "$sqlite_home" = "$codex_home" ]; then
    echo "The medical workspace SQLite home must be separate from CODEX_HOME." >&2
    exit 2
fi

chmod 700 "$sqlite_home"
sqlite_home_toml=$(python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$sqlite_home")

export FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION=synthetic
export CODEX_SQLITE_HOME="$sqlite_home"

if [ -t 0 ] && [ -t 1 ]; then
    stty cols 160 rows 45 2>/dev/null || true
fi

echo "Opening the synthetic medical workspace with private SQLite state at: $sqlite_home"
echo "Enter /workspacemedical after Codex opens."

cd "$repo_root/codex-rs"
exec cargo run --bin codex -- -c "sqlite_home=$sqlite_home_toml"
