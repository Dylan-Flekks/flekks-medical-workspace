#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
home=${HOME:?HOME must be set to launch the medical workspace}
sqlite_home=${FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME:-"$home/.codex/flekks-medical-synthetic"}
codex_home=${CODEX_HOME:-"$home/.codex"}

minimum_free_kib=$((10 * 1024 * 1024))
recommended_free_kib=$((30 * 1024 * 1024))
disk_warning=0
terminal_rows=
terminal_cols=
lock_dir=
lock_owned=0

restore_terminal_size() {
    if [ -n "$terminal_rows" ] && [ -n "$terminal_cols" ]; then
        stty rows "$terminal_rows" cols "$terminal_cols" 2>/dev/null || true
    fi
}

cleanup() {
    restore_terminal_size
    if [ "$lock_owned" -eq 1 ] && [ -n "$lock_dir" ] && [ -d "$lock_dir" ]; then
        lock_pid=$(cat "$lock_dir/pid" 2>/dev/null || true)
        if [ "$lock_pid" = "$$" ]; then
            rm -f "$lock_dir/pid" "$lock_dir/repository"
            rmdir "$lock_dir" 2>/dev/null || true
        fi
    fi
}

trap cleanup EXIT
trap 'exit 130' HUP INT TERM

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

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required to resolve the isolated medical-workspace paths." >&2
    exit 2
fi

umask 077
store_tool="$repo_root/scripts/medical_workspace_store.py"
if ! validated_sqlite_home=$(python3 "$store_tool" \
    --store "$sqlite_home" \
    --codex-home "$codex_home" \
    validate-path); then
    echo "Medical workspace launch blocked before creating or changing the synthetic store." >&2
    exit 2
fi

# Use the validated canonical path so a symlink in the user-supplied path cannot
# redirect initialization or chmod. A preexisting leaf must already carry the
# exact marker; only the helper may atomically initialize a nonexistent leaf.
sqlite_home=$validated_sqlite_home
store_preexisted=0
if [ -e "$sqlite_home" ] || [ -L "$sqlite_home" ]; then
    store_preexisted=1
fi
if [ "$store_preexisted" -eq 1 ]; then
    if ! python3 "$store_tool" \
        --store "$sqlite_home" \
        --codex-home "$codex_home" \
        validate-marker >/dev/null; then
        echo "Medical workspace launch blocked: a preexisting store must already have the exact synthetic-store marker." >&2
        exit 2
    fi
else
    if ! python3 "$store_tool" \
        --store "$sqlite_home" \
        --codex-home "$codex_home" \
        initialize-store >/dev/null; then
        echo "Medical workspace launch blocked: the new synthetic-store leaf could not be initialized atomically." >&2
        exit 2
    fi
fi

if ! revalidated_sqlite_home=$(python3 "$store_tool" \
    --store "$sqlite_home" \
    --codex-home "$codex_home" \
    validate-path); then
    echo "Medical workspace launch blocked before changing the synthetic store." >&2
    exit 2
fi
if [ "$revalidated_sqlite_home" != "$sqlite_home" ]; then
    echo "Medical workspace launch blocked: the synthetic store path changed during validation." >&2
    exit 2
fi
if ! python3 "$store_tool" \
    --store "$sqlite_home" \
    --codex-home "$codex_home" \
    validate-marker >/dev/null; then
    echo "Medical workspace launch blocked: the synthetic-store marker changed during validation." >&2
    exit 2
fi

codex_home=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$codex_home")

chmod 700 "$sqlite_home"
sqlite_home_toml=$(python3 -c 'import json, sys; print(json.dumps(sys.argv[1]))' "$sqlite_home")

marker_file="$sqlite_home/.flekks-medical-synthetic-store"
chmod 600 "$marker_file"

lock_dir="$sqlite_home/.flekks-medical-workspace.lock"
if ! mkdir "$lock_dir" 2>/dev/null; then
    existing_pid=$(cat "$lock_dir/pid" 2>/dev/null || true)
    echo "Medical workspace launch blocked: this synthetic store is already locked${existing_pid:+ by process $existing_pid}." >&2
    echo "Verify that no medical workspace is using $sqlite_home before removing a stale lock." >&2
    exit 2
fi
lock_owned=1
printf '%s\n' "$$" >"$lock_dir/pid"
printf '%s\n' "$repo_root" >"$lock_dir/repository"
chmod 700 "$lock_dir"
chmod 600 "$lock_dir/pid" "$lock_dir/repository"

print_disk_diagnostics() {
    current_target="$repo_root/codex-rs/target"
    echo "The launcher did not delete any files." >&2
    echo "Inspect likely build storage before choosing what to remove:" >&2
    echo "  df -h \"$repo_root\" \"$sqlite_home\"" >&2
    echo "  du -sh \"$current_target\" \"$home/.cargo/registry\" \"$home/.cargo/git\" 2>/dev/null" >&2
    echo "  find \"$home\" -type d -path '*/codex-rs/target' -prune -exec du -sh {} \\; 2>/dev/null | sort -h" >&2
    echo "After inspecting the paths, clean only a checkout whose build artifacts you intend to rebuild:" >&2
    echo "  cargo clean --manifest-path \"$repo_root/codex-rs/Cargo.toml\"" >&2
}

check_free_space() {
    label=$1
    path=$2
    available_kib=$(df -Pk "$path" 2>/dev/null | awk 'NR == 2 { print $4 }')
    case "$available_kib" in
        ''|*[!0-9]*)
            echo "Could not determine available disk space for $label at: $path" >&2
            print_disk_diagnostics
            exit 2
            ;;
    esac

    available_gib=$(awk -v available_kib="$available_kib" 'BEGIN { printf "%.1f", available_kib / 1048576 }')
    if [ "$available_kib" -lt "$minimum_free_kib" ]; then
        echo "Medical workspace launch blocked: $label has only $available_gib GiB free at $path; at least 10 GiB is required." >&2
        print_disk_diagnostics
        exit 2
    fi
    if [ "$available_kib" -lt "$recommended_free_kib" ]; then
        echo "Warning: $label has $available_gib GiB free at $path; 30 GiB or more is recommended for the first Rust build." >&2
        disk_warning=1
    else
        echo "Disk preflight: $label has $available_gib GiB free."
    fi
}

# The Rust build output and isolated SQLite store may live on different volumes,
# so both locations must have enough room before a large first build begins.
check_free_space "the source/build volume" "$repo_root/codex-rs"
check_free_space "the synthetic SQLite volume" "$sqlite_home"
if [ "$disk_warning" -eq 1 ]; then
    print_disk_diagnostics
fi

export FLEKKS_MEDICAL_WORKSPACE_DATA_CLASSIFICATION=synthetic
export CODEX_SQLITE_HOME="$sqlite_home"
export CARGO_INCREMENTAL=0

if [ -t 0 ] && [ -t 1 ] && command -v stty >/dev/null 2>&1; then
    terminal_size=$(stty size 2>/dev/null || true)
    case "$terminal_size" in
        *' '*)
            terminal_rows=${terminal_size%% *}
            terminal_cols=${terminal_size##* }
            stty cols 160 rows 45 2>/dev/null || true
            ;;
    esac
fi

echo "Opening Codex with synthetic-only medical SQLite state at: $sqlite_home"
echo "The first build compiles the Codex workspace and can take several minutes."
echo "Enter /workspacemedical after Codex opens."

cd "$repo_root/codex-rs"
cargo run --profile dev-small --bin codex -- -c "sqlite_home=$sqlite_home_toml" "$@"
