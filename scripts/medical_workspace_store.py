#!/usr/bin/env python3
"""Inspect or safely purge the dedicated synthetic medical-workspace store."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import sqlite3
import stat
import sys
from typing import Any, Iterator


MARKER_NAME = ".flekks-medical-synthetic-store"
MARKER_VALUE = "flekks-medical-synthetic-store-v1"
LOCK_NAME = ".flekks-medical-workspace.lock"
WORKSPACE_DB_NAME = "workspace_1.sqlite"
DB_BASE_NAMES = {
    "goals_1.sqlite",
    "logs_2.sqlite",
    "memories_1.sqlite",
    "state_5.sqlite",
    "thread_history_1.sqlite",
    WORKSPACE_DB_NAME,
}
EXPECTED_WORKSPACE_TABLES = {
    "workspace_agent_results",
    "workspace_agent_run_sources",
    "workspace_agent_runs",
    "workspace_artifact_derivatives",
    "workspace_audit_events",
    "workspace_chart_commits",
    "workspace_client_contacts",
    "workspace_client_coverages",
    "workspace_clients",
    "workspace_context_clips",
    "workspace_context_packets",
    "workspace_coverage_card_verifications",
    "workspace_coverages",
    "workspace_data_policy",
    "workspace_documents",
    "workspace_draft_checkpoints",
    "workspace_draft_sessions",
    "workspace_encounters",
    "workspace_guide_runs",
    "workspace_note_addenda",
    "workspace_note_proposal_decisions",
    "workspace_note_proposals",
    "workspace_note_revisions",
    "workspace_note_signatures",
    "workspace_notes",
    "workspace_patient_safety_items",
    "workspace_tasks",
}
ALLOWED_TOP_LEVEL_NAMES = DB_BASE_NAMES | {
    MARKER_NAME,
    LOCK_NAME,
    "db-backups",
}


class StoreError(RuntimeError):
    """A fail-closed store validation error."""


def parse_args(argv: list[str]) -> argparse.Namespace:
    home = Path(os.environ.get("HOME", "~")).expanduser()
    parser = argparse.ArgumentParser(
        description=(
            "Inspect or purge only the dedicated synthetic Flekks medical-workspace "
            "SQLite store. The default operation is read-only status."
        )
    )
    parser.add_argument(
        "--store",
        default=os.environ.get(
            "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME",
            str(home / ".codex" / "flekks-medical-synthetic"),
        ),
        help="Absolute synthetic SQLite home (defaults to the launcher path).",
    )
    parser.add_argument(
        "--codex-home",
        default=os.environ.get("CODEX_HOME", str(home / ".codex")),
        help=argparse.SUPPRESS,
    )
    subparsers = parser.add_subparsers(dest="command")
    subparsers.add_parser(
        "validate-path",
        help="Validate and print the canonical store path without creating it.",
    )
    subparsers.add_parser(
        "validate-marker",
        help="Validate an existing store marker without changing the store.",
    )
    subparsers.add_parser(
        "initialize-store",
        help="Atomically create and mark a store whose leaf does not exist.",
    )
    subparsers.add_parser(
        "status", help="Run read-only policy, SQLite, and permission checks."
    )
    purge = subparsers.add_parser(
        "purge",
        help="Inventory the store; delete only with exact confirmation and a receipt path.",
    )
    purge.add_argument(
        "--confirm",
        help="Exact canonical store path. Omit this option for a dry run.",
    )
    purge.add_argument(
        "--receipt",
        help=(
            "Absolute JSON start-receipt path outside the store; successful deletion "
            "also creates a collision-safe .complete companion."
        ),
    )
    return parser.parse_args(argv)


def is_same_or_ancestor(candidate: Path, protected: Path) -> bool:
    try:
        protected.relative_to(candidate)
    except ValueError:
        return False
    return True


def resolve_store(raw_store: str, raw_codex_home: str) -> Path:
    entered = Path(raw_store).expanduser()
    if not entered.is_absolute():
        raise StoreError("the synthetic store path must be absolute")
    if entered.is_symlink():
        raise StoreError("the synthetic store path itself must not be a symlink")
    store = entered.resolve(strict=False)
    codex_home = Path(raw_codex_home).expanduser()
    if not codex_home.is_absolute():
        raise StoreError("CODEX_HOME must be absolute")
    codex_home = codex_home.resolve(strict=False)
    home = Path.home().resolve()
    repo_root = Path(__file__).resolve().parents[1]
    protected = {Path("/"), home, home / ".codex", codex_home, repo_root}
    for path in protected:
        path = path.resolve(strict=False)
        if store == path or is_same_or_ancestor(store, path):
            raise StoreError(
                f"refusing store path {store}: it is or contains protected path {path}"
            )
    if repo_root == store or repo_root in store.parents:
        raise StoreError(
            f"refusing store path {store}: the synthetic store must be outside the "
            f"repository {repo_root}"
        )
    if store == codex_home:
        raise StoreError("the synthetic store must be separate from CODEX_HOME")
    return store


def file_mode(path: Path) -> int:
    return stat.S_IMODE(path.stat(follow_symlinks=False).st_mode)


def fsync_directory(directory: Path) -> None:
    flags = os.O_RDONLY
    flags |= getattr(os, "O_DIRECTORY", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(directory, flags)
    except OSError as error:
        raise StoreError(
            f"could not open directory for durability sync: {error}"
        ) from error
    try:
        os.fsync(descriptor)
    except OSError as error:
        raise StoreError(f"could not sync directory metadata: {error}") from error
    finally:
        os.close(descriptor)


def read_marker(store: Path) -> None:
    marker = store / MARKER_NAME
    if marker.is_symlink() or not marker.is_file():
        raise StoreError(f"missing or unsafe synthetic-store marker: {marker}")
    try:
        marker_value = marker.read_bytes()
    except OSError as error:
        raise StoreError(f"could not read synthetic-store marker: {marker}") from error
    if marker_value != (MARKER_VALUE + "\n").encode():
        raise StoreError(f"invalid synthetic-store marker: {marker}")


def create_marker_exclusive(store: Path) -> None:
    marker = store / MARKER_NAME
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(marker, flags, 0o600)
    except FileExistsError as error:
        raise StoreError(
            f"synthetic-store marker already exists; refusing initialization: {marker}"
        ) from error
    except OSError as error:
        raise StoreError(f"could not create synthetic-store marker: {error}") from error
    with os.fdopen(descriptor, "w", encoding="utf-8") as output:
        output.write(MARKER_VALUE + "\n")
        output.flush()
        os.fsync(output.fileno())
    fsync_directory(store)


def initialize_store(store: Path) -> None:
    if store.is_symlink() or store.exists():
        raise StoreError(
            f"synthetic store leaf already exists; refusing initialization: {store}"
        )
    try:
        store.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        store.mkdir(mode=0o700)
    except FileExistsError as error:
        raise StoreError(
            f"synthetic store leaf appeared during initialization; refusing it: {store}"
        ) from error
    except OSError as error:
        raise StoreError(f"could not create synthetic store: {error}") from error
    create_marker_exclusive(store)
    fsync_directory(store.parent)


def open_workspace_db(store: Path) -> sqlite3.Connection:
    db_path = store / WORKSPACE_DB_NAME
    if db_path.is_symlink() or not db_path.is_file():
        raise StoreError(f"missing or unsafe workspace database: {db_path}")
    try:
        uri = f"{db_path.resolve(strict=True).as_uri()}?mode=ro"
    except OSError as error:
        raise StoreError(f"could not resolve workspace database: {error}") from error
    try:
        connection = sqlite3.connect(uri, uri=True)
        connection.execute("PRAGMA query_only = ON")
    except sqlite3.Error as error:
        raise StoreError(
            f"could not open workspace database read-only: {error}"
        ) from error
    return connection


def read_policy(connection: sqlite3.Connection) -> dict[str, Any]:
    try:
        rows = connection.execute(
            "SELECT singleton_id, schema_version, data_classification, "
            "classified_at_ms, classified_by FROM workspace_data_policy"
        ).fetchall()
    except sqlite3.Error as error:
        raise StoreError(f"workspace data policy is unavailable: {error}") from error
    if len(rows) != 1:
        raise StoreError(
            f"workspace data policy must contain one row; found {len(rows)}"
        )
    singleton, schema_version, classification, classified_at_ms, classified_by = rows[0]
    if singleton != 1 or schema_version != 1:
        raise StoreError("workspace data policy has a noncanonical identity or schema")
    if classification != "synthetic":
        raise StoreError(
            f"workspace data policy is {classification!r}; only synthetic stores are supported"
        )
    if not isinstance(classified_at_ms, int) or classified_at_ms < 0:
        raise StoreError(
            "synthetic workspace policy has an invalid classification time"
        )
    if not isinstance(classified_by, str) or not classified_by.strip():
        raise StoreError("synthetic workspace policy has no attestation source")
    return {
        "schema_version": schema_version,
        "classification": classification,
        "classified_at_ms": classified_at_ms,
        "classified_by": classified_by,
    }


def sqlite_health(connection: sqlite3.Connection) -> tuple[str, int, set[str]]:
    try:
        quick_check_rows = connection.execute("PRAGMA quick_check").fetchall()
        foreign_key_rows = connection.execute("PRAGMA foreign_key_check").fetchall()
        tables = {
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_schema "
                "WHERE type = 'table' AND name LIKE 'workspace_%'"
            )
        }
    except sqlite3.Error as error:
        raise StoreError(f"SQLite health check failed: {error}") from error
    quick_check = "; ".join(str(row[0]) for row in quick_check_rows)
    return quick_check, len(foreign_key_rows), tables


def lock_description(store: Path) -> tuple[bool, str]:
    lock = store / LOCK_NAME
    if lock.is_symlink():
        return True, "unsafe lock entry"
    if not lock.exists():
        return False, "none"
    if not lock.is_dir():
        return True, "unsafe lock entry"
    pid_text = "unknown"
    try:
        pid_text = (lock / "pid").read_text(encoding="utf-8").strip()
    except (OSError, UnicodeError):
        pass
    if pid_text.isdigit():
        try:
            os.kill(int(pid_text), 0)
        except ProcessLookupError:
            return True, f"stale lock for process {pid_text}"
        except PermissionError:
            return True, f"active lock for process {pid_text} (ownership unknown)"
        return True, f"active lock for process {pid_text}"
    return True, f"lock with invalid process id {pid_text!r}"


@contextmanager
def exclusive_purge_lock(store: Path) -> Iterator[None]:
    """Acquire the launcher's directory lock and retain it through deletion."""
    if not store.exists() or not store.is_dir():
        raise StoreError(f"synthetic store does not exist: {store}")
    lock = store / LOCK_NAME
    try:
        lock.mkdir(mode=0o700)
    except FileExistsError as error:
        _, description = lock_description(store)
        raise StoreError(
            "purge refused because the synthetic store is already locked "
            f"({description})"
        ) from error
    except OSError as error:
        raise StoreError(
            f"could not acquire the synthetic-store purge lock: {error}"
        ) from error

    pid_value = str(os.getpid())
    operation_value = "purge"
    try:
        pid_file = lock / "pid"
        operation_file = lock / "operation"
        pid_file.write_text(pid_value + "\n", encoding="utf-8")
        operation_file.write_text(operation_value + "\n", encoding="utf-8")
        os.chmod(pid_file, 0o600)
        os.chmod(operation_file, 0o600)
        yield
    finally:
        # A successful purge removes the lock with the store. On an error, remove
        # only the exact lock this process created; ambiguous lock state is left
        # in place for a human to inspect.
        if lock.is_dir() and not lock.is_symlink():
            try:
                owns_lock = (lock / "pid").read_text(
                    encoding="utf-8"
                ).strip() == pid_value and (lock / "operation").read_text(
                    encoding="utf-8"
                ).strip() == operation_value
            except (OSError, UnicodeError):
                owns_lock = False
            if owns_lock:
                try:
                    (lock / "pid").unlink()
                    (lock / "operation").unlink()
                    lock.rmdir()
                except OSError:
                    pass


def assert_same_device(root_device: int, path: Path, device: int) -> None:
    if device != root_device:
        raise StoreError(
            "store contains a nested mount or cross-device descendant; refusing to "
            f"traverse or delete it: {path}"
        )


def iter_inventory(store: Path) -> list[dict[str, Any]]:
    inventory: list[dict[str, Any]] = []
    try:
        root_metadata = store.stat(follow_symlinks=False)
    except OSError as error:
        raise StoreError(f"could not inspect synthetic store: {error}") from error
    root_device = root_metadata.st_dev

    def walk(directory: Path) -> None:
        try:
            with os.scandir(directory) as entries:
                children = sorted((Path(entry.path) for entry in entries), key=str)
        except OSError as error:
            raise StoreError(f"could not inventory synthetic store: {error}") from error
        for path in children:
            try:
                metadata = path.stat(follow_symlinks=False)
            except OSError as error:
                raise StoreError(
                    f"could not inspect store entry {path}: {error}"
                ) from error
            assert_same_device(root_device, path, metadata.st_dev)
            relative = path.relative_to(store).as_posix()
            if stat.S_ISLNK(metadata.st_mode):
                kind = "symlink"
                size = 0
            elif stat.S_ISDIR(metadata.st_mode):
                kind = "directory"
                size = 0
            elif stat.S_ISREG(metadata.st_mode):
                kind = "file"
                size = metadata.st_size
            else:
                kind = "other"
                size = 0
            inventory.append({"path": relative, "kind": kind, "bytes": size})
            if kind == "directory":
                walk(path)

    walk(store)
    return inventory


def unexpected_top_level_entries(store: Path) -> list[str]:
    unexpected: list[str] = []
    for path in store.iterdir():
        name = path.name
        allowed = name in ALLOWED_TOP_LEVEL_NAMES or any(
            name == f"{base}{suffix}"
            for base in DB_BASE_NAMES
            for suffix in ("-journal", "-shm", "-wal")
        )
        if not allowed:
            unexpected.append(name)
    return sorted(unexpected)


def validate_store(
    store: Path,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[str]]:
    if store.is_symlink() or not store.exists() or not store.is_dir():
        raise StoreError(f"synthetic store does not exist: {store}")
    # Inventory first so a nested mount is rejected before any recursive store
    # traversal or database access can cross the deletion boundary.
    inventory = iter_inventory(store)
    unsafe = [
        entry["path"] for entry in inventory if entry["kind"] in {"symlink", "other"}
    ]
    if unsafe:
        raise StoreError(f"store contains unsafe filesystem entries: {unsafe}")
    read_marker(store)
    connection = open_workspace_db(store)
    try:
        policy = read_policy(connection)
        quick_check, foreign_key_failures, actual_tables = sqlite_health(connection)
    finally:
        connection.close()
    if quick_check != "ok":
        raise StoreError(f"SQLite quick_check did not return ok: {quick_check}")
    if foreign_key_failures:
        raise StoreError(
            f"SQLite foreign_key_check found {foreign_key_failures} violation(s)"
        )
    if actual_tables != EXPECTED_WORKSPACE_TABLES:
        missing = sorted(EXPECTED_WORKSPACE_TABLES - actual_tables)
        extra = sorted(actual_tables - EXPECTED_WORKSPACE_TABLES)
        raise StoreError(
            f"workspace table-name inventory mismatch; missing={missing}, extra={extra}"
        )
    return policy, inventory, unexpected_top_level_entries(store)


def status(store: Path) -> int:
    policy, inventory, unexpected = validate_store(store)
    lock_exists, lock = lock_description(store)
    store_mode = file_mode(store)
    overly_open = [
        entry["path"]
        for entry in inventory
        if entry["kind"] in {"file", "directory"}
        and file_mode(store / entry["path"]) & 0o077
    ]
    total_bytes = sum(entry["bytes"] for entry in inventory)
    free_bytes = shutil.disk_usage(store).free
    print(f"Store: {store}")
    print(
        f"Classification: {policy['classification']} (schema {policy['schema_version']})"
    )
    print(f"Attested by: {policy['classified_by']}")
    print(f"Workspace database: {WORKSPACE_DB_NAME}")
    print("Workspace SQLite quick_check: ok")
    print("Workspace SQLite foreign_key_check: ok")
    print(f"Directory mode: {store_mode:03o}")
    print(f"Lock: {lock}")
    print(f"Stored bytes: {total_bytes}")
    print(f"Free bytes: {free_bytes}")
    warnings: list[str] = []
    if store_mode & 0o077:
        warnings.append("store directory grants group or other permissions")
    if overly_open:
        warnings.append(f"entries grant group or other permissions: {overly_open}")
    if unexpected:
        warnings.append(f"unexpected top-level entries: {unexpected}")
    if lock_exists:
        warnings.append(lock)
    if warnings:
        for warning in warnings:
            print(f"WARNING: {warning}", file=sys.stderr)
        return 1
    return 0


def receipt_payload(
    store: Path,
    policy: dict[str, Any],
    inventory: list[dict[str, Any]],
    status_value: str,
) -> dict[str, Any]:
    canonical_inventory = json.dumps(inventory, sort_keys=True, separators=(",", ":"))
    return {
        "format": "flekks-medical-synthetic-purge-receipt-v1",
        "status": status_value,
        "store": str(store),
        "classification": policy["classification"],
        "policy_schema_version": policy["schema_version"],
        "inventory_sha256": hashlib.sha256(canonical_inventory.encode()).hexdigest(),
        "entry_count": len(inventory),
        "total_bytes": sum(entry["bytes"] for entry in inventory),
        "recorded_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    }


def write_receipt(
    path: Path,
    payload: dict[str, Any],
) -> None:
    """Install a private receipt without following or overwriting unknown files."""
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}-{secrets.token_hex(8)}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    flags |= getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(temporary, flags, 0o600)
    except OSError as error:
        raise StoreError(
            f"could not create a private purge receipt: {error}"
        ) from error

    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as output:
            json.dump(payload, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        try:
            os.link(temporary, path, follow_symlinks=False)
        except FileExistsError as error:
            raise StoreError(
                f"purge receipt target already exists; refusing to overwrite it: {path}"
            ) from error
        except OSError as error:
            raise StoreError(f"could not install the purge receipt: {error}") from error
        temporary.unlink()
        fsync_directory(path.parent)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def completion_receipt_path(receipt: Path) -> Path:
    return receipt.with_name(f"{receipt.name}.complete")


def report_purge_inventory(
    store: Path,
    inventory: list[dict[str, Any]],
    lock: str,
) -> None:
    print(f"Synthetic store: {store}")
    print(f"Entries: {len(inventory)}")
    print(f"Bytes: {sum(entry['bytes'] for entry in inventory)}")
    print(f"Lock: {lock}")


def validate_receipt_path(store: Path, receipt_value: str | None) -> Path:
    if receipt_value is None:
        raise StoreError("--receipt is required for an actual purge")
    entered = Path(receipt_value).expanduser()
    if not entered.is_absolute():
        raise StoreError("--receipt must be an absolute path")
    parent = entered.parent
    if parent.is_symlink() or not parent.is_dir():
        raise StoreError(
            "the purge receipt parent must already exist as a non-symlink directory"
        )
    try:
        parent = parent.resolve(strict=True)
    except OSError as error:
        raise StoreError(
            f"could not resolve the purge receipt parent: {error}"
        ) from error
    receipt = parent / entered.name
    if receipt == store or is_same_or_ancestor(store, receipt):
        raise StoreError("the purge receipt must be outside the store being removed")
    for target in (receipt, completion_receipt_path(receipt)):
        if target.is_symlink() or target.exists():
            raise StoreError(
                f"purge receipt target must not already exist or be a symlink: {target}"
            )
    return receipt


def purge(store: Path, confirmation: str | None, receipt_value: str | None) -> int:
    if confirmation is None:
        policy, inventory, unexpected = validate_store(store)
        del policy
        lock_exists, lock = lock_description(store)
        report_purge_inventory(store, inventory, lock)
        if unexpected:
            raise StoreError(
                "purge refused because the store has unexpected top-level entries: "
                f"{unexpected}"
            )
        if lock_exists:
            raise StoreError(
                f"purge refused while a launcher lock exists ({lock}); verify the process "
                "and remove only a confirmed stale lock"
            )
        print("Dry run only; no files were deleted.")
        print("To purge, rerun with both:")
        print(f"  --confirm {store!s}")
        print("  --receipt /absolute/path/outside-the-store/purge-receipt.json")
        return 0

    if confirmation != str(store):
        raise StoreError(
            "--confirm must exactly match the canonical store path printed by the dry run"
        )
    receipt = validate_receipt_path(store, receipt_value)

    # Acquiring the same atomic directory lock as the launcher closes the race
    # between checking for an active workspace and deleting the store. All
    # mutable work is performed only after the lock is held and validation is
    # repeated from disk.
    with exclusive_purge_lock(store):
        policy, inventory, unexpected = validate_store(store)
        report_purge_inventory(store, inventory, "owned purge lock")
        if unexpected:
            raise StoreError(
                "purge refused because the store has unexpected top-level entries: "
                f"{unexpected}"
            )
        completion_receipt = completion_receipt_path(receipt)
        started_payload = receipt_payload(store, policy, inventory, "started")
        started_payload["completion_receipt"] = str(completion_receipt)
        write_receipt(
            receipt,
            started_payload,
        )
        final_inventory = iter_inventory(store)
        if final_inventory != inventory:
            raise StoreError(
                "purge refused because the store changed after validation; run a new dry run"
            )
        shutil.rmtree(store)
        if store.exists():
            raise StoreError(f"purge verification failed; store still exists: {store}")
        completed_payload = receipt_payload(store, policy, inventory, "complete")
        completed_payload["started_receipt"] = str(receipt)
        write_receipt(
            completion_receipt,
            completed_payload,
        )

    print(f"Purge complete. Started receipt: {receipt}")
    print(f"Completion receipt: {completion_receipt_path(receipt)}")
    print(
        "This removed only the dedicated SQLite home. Review ordinary CODEX_HOME "
        "separately for retained Codex rollouts or logs; this utility never deletes it."
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        store = resolve_store(args.store, args.codex_home)
        command = args.command or "status"
        if command == "validate-path":
            print(store)
            return 0
        if command == "validate-marker":
            read_marker(store)
            print(store)
            return 0
        if command == "initialize-store":
            initialize_store(store)
            print(store)
            return 0
        if command == "status":
            return status(store)
        return purge(store, args.confirm, args.receipt)
    except StoreError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
