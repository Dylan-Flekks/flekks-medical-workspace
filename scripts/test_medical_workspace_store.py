from __future__ import annotations

import importlib.util
import io
import json
import os
from pathlib import Path
import sqlite3
import stat
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock


SCRIPT = Path(__file__).with_name("medical_workspace_store.py")
LAUNCHER = Path(__file__).with_name("run_medical_workspace.sh")
SPEC = importlib.util.spec_from_file_location("medical_workspace_store", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
store_tool = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(store_tool)


class MedicalWorkspaceStoreTests(unittest.TestCase):
    def create_store(self, root: Path) -> Path:
        store = root / "synthetic-store"
        store.mkdir(mode=0o700)
        marker = store / store_tool.MARKER_NAME
        marker.write_text(store_tool.MARKER_VALUE + "\n", encoding="utf-8")
        os.chmod(marker, 0o600)
        db_path = store / store_tool.WORKSPACE_DB_NAME
        connection = sqlite3.connect(db_path)
        try:
            for table in sorted(store_tool.EXPECTED_WORKSPACE_TABLES):
                if table == "workspace_data_policy":
                    connection.execute(
                        "CREATE TABLE workspace_data_policy ("
                        "singleton_id INTEGER, schema_version INTEGER, "
                        "data_classification TEXT, classified_at_ms INTEGER, "
                        "classified_by TEXT)"
                    )
                else:
                    connection.execute(f"CREATE TABLE {table} (id TEXT)")
            connection.execute(
                "INSERT INTO workspace_data_policy VALUES (1, 1, 'synthetic', 1, 'test')"
            )
            connection.commit()
        finally:
            connection.close()
        os.chmod(db_path, 0o600)
        return store

    def test_status_accepts_a_private_canonical_synthetic_store(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)

            self.assertEqual(store_tool.status(store), 0)
            self.assertTrue(store.exists())

    def test_status_opens_question_and_fragment_paths_read_only(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "question?fragment#"
            root.mkdir()
            store = self.create_store(root)

            self.assertEqual(store_tool.status(store), 0)
            self.assertTrue(store.exists())

    def test_purge_is_dry_run_without_exact_confirmation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)

            self.assertEqual(store_tool.purge(store, None, None), 0)
            self.assertTrue(store.exists())
            self.assertFalse((store / store_tool.LOCK_NAME).exists())

    def test_purge_writes_receipt_and_removes_only_the_store(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            sibling = root / "keep.txt"
            sibling.write_text("keep", encoding="utf-8")
            receipt_parent = root / "receipts"
            receipt_parent.mkdir(mode=0o700)
            receipt = receipt_parent / "purge.json"

            self.assertEqual(
                store_tool.purge(store, str(store), str(receipt)),
                0,
            )

            self.assertFalse(store.exists())
            self.assertEqual(sibling.read_text(encoding="utf-8"), "keep")
            started_payload = json.loads(receipt.read_text(encoding="utf-8"))
            completion = store_tool.completion_receipt_path(receipt)
            completed_payload = json.loads(completion.read_text(encoding="utf-8"))
            self.assertEqual(started_payload["status"], "started")
            self.assertEqual(
                started_payload["completion_receipt"],
                str(completion.resolve()),
            )
            self.assertEqual(completed_payload["status"], "complete")
            self.assertEqual(completed_payload["classification"], "synthetic")
            self.assertEqual(
                completed_payload["started_receipt"], str(receipt.resolve())
            )
            self.assertEqual(stat.S_IMODE(receipt.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(completion.stat().st_mode), 0o600)

    def test_purge_syncs_the_trusted_parent_after_each_receipt_install(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            receipt = root / "purge.json"
            original_sync = store_tool.fsync_directory
            synced_directories: list[Path] = []

            def record_sync(directory: Path):
                synced_directories.append(directory)
                return original_sync(directory)

            with mock.patch.object(
                store_tool,
                "fsync_directory",
                side_effect=record_sync,
            ):
                self.assertEqual(
                    store_tool.purge(store, str(store), str(receipt)),
                    0,
                )

            self.assertEqual(
                synced_directories,
                [receipt.parent.resolve(), receipt.parent.resolve()],
            )

    def test_actual_purge_holds_owned_lock_during_revalidation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            receipt = root / "purge.json"
            original_validate = store_tool.validate_store
            lock_observations: list[bool] = []

            def validate_while_observing(path: Path):
                lock_observations.append((path / store_tool.LOCK_NAME).is_dir())
                return original_validate(path)

            with mock.patch.object(
                store_tool,
                "validate_store",
                side_effect=validate_while_observing,
            ):
                self.assertEqual(
                    store_tool.purge(store, str(store), str(receipt)),
                    0,
                )

            self.assertEqual(lock_observations, [True])

    def test_actual_purge_refuses_an_existing_launcher_lock(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            lock = store / store_tool.LOCK_NAME
            lock.mkdir(mode=0o700)
            (lock / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")
            receipt = root / "purge.json"

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(receipt))

            self.assertTrue(store.exists())
            self.assertFalse(receipt.exists())

    def test_actual_purge_refuses_an_unknown_lock_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            (store / store_tool.LOCK_NAME).write_text("unknown", encoding="utf-8")

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(root / "purge.json"))

            self.assertTrue(store.exists())

    def test_actual_purge_refuses_a_cross_device_descendant(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            nested_mount = store / "db-backups"
            nested_mount.mkdir()
            receipt = root / "purge.json"
            original_check = store_tool.assert_same_device

            def report_nested_mount(root_device: int, path: Path, device: int):
                if path == nested_mount:
                    device = root_device + 1
                return original_check(root_device, path, device)

            with mock.patch.object(
                store_tool,
                "assert_same_device",
                side_effect=report_nested_mount,
            ):
                with self.assertRaises(store_tool.StoreError):
                    store_tool.purge(store, str(store), str(receipt))

            self.assertTrue(store.exists())
            self.assertFalse((store / store_tool.LOCK_NAME).exists())
            self.assertFalse(receipt.exists())
            self.assertFalse(store_tool.completion_receipt_path(receipt).exists())

    def test_purge_refuses_unexpected_top_level_content(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            (store / "unrelated.txt").write_text("do not remove", encoding="utf-8")

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(root / "receipt.json"))

            self.assertTrue(store.exists())

    def test_purge_refuses_an_existing_receipt_without_changing_it(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            receipt = root / "purge.json"
            receipt.write_text("keep\n", encoding="utf-8")

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(receipt))

            self.assertTrue(store.exists())
            self.assertEqual(receipt.read_text(encoding="utf-8"), "keep\n")

    def test_purge_refuses_an_existing_completion_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            receipt = root / "purge.json"
            completion = store_tool.completion_receipt_path(receipt)
            completion.write_text("keep\n", encoding="utf-8")

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(receipt))

            self.assertTrue(store.exists())
            self.assertFalse(receipt.exists())
            self.assertEqual(completion.read_text(encoding="utf-8"), "keep\n")

    def test_purge_refuses_a_receipt_created_after_path_validation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            receipt = root / "purge.json"

            def collide_after_validation(*_args):
                receipt.write_text("racer\n", encoding="utf-8")
                return receipt

            with mock.patch.object(
                store_tool,
                "validate_receipt_path",
                side_effect=collide_after_validation,
            ):
                with self.assertRaises(store_tool.StoreError):
                    store_tool.purge(store, str(store), str(receipt))

            self.assertTrue(store.exists())
            self.assertFalse((store / store_tool.LOCK_NAME).exists())
            self.assertEqual(receipt.read_text(encoding="utf-8"), "racer\n")

    def test_purge_refuses_a_symlink_receipt_or_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            target = root / "target.json"
            receipt_link = root / "receipt-link.json"
            receipt_link.symlink_to(target)

            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(receipt_link))
            self.assertFalse(target.exists())

            actual_parent = root / "actual-parent"
            actual_parent.mkdir()
            parent_link = root / "parent-link"
            parent_link.symlink_to(actual_parent, target_is_directory=True)
            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(
                    store,
                    str(store),
                    str(parent_link / "receipt.json"),
                )
            self.assertFalse((actual_parent / "receipt.json").exists())

            receipt = root / "receipt.json"
            completion_link = store_tool.completion_receipt_path(receipt)
            completion_target = root / "completion-target.json"
            completion_link.symlink_to(completion_target)
            with self.assertRaises(store_tool.StoreError):
                store_tool.purge(store, str(store), str(receipt))
            self.assertFalse(receipt.exists())
            self.assertFalse(completion_target.exists())

    def test_resolve_store_refuses_codex_home_and_its_ancestors(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            codex_home = Path(temp) / "codex-home"
            codex_home.mkdir()

            with self.assertRaises(store_tool.StoreError):
                store_tool.resolve_store(str(codex_home), str(codex_home))
            with self.assertRaises(store_tool.StoreError):
                store_tool.resolve_store(str(codex_home.parent), str(codex_home))

    def test_status_refuses_unclassified_or_corrupt_workspace_databases(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            db_path = store / store_tool.WORKSPACE_DB_NAME
            connection = sqlite3.connect(db_path)
            try:
                connection.execute(
                    "UPDATE workspace_data_policy SET data_classification = 'unclassified'"
                )
                connection.commit()
            finally:
                connection.close()

            with self.assertRaises(store_tool.StoreError):
                store_tool.status(store)

            db_path.write_bytes(b"not a sqlite database")
            with self.assertRaises(store_tool.StoreError):
                store_tool.status(store)

    def test_status_reports_overly_broad_entry_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = self.create_store(root)
            marker = store / store_tool.MARKER_NAME
            os.chmod(marker, 0o644)
            output = io.StringIO()
            errors = io.StringIO()

            with redirect_stdout(output), redirect_stderr(errors):
                result = store_tool.status(store)

            self.assertEqual(result, 1)
            self.assertIn("entries grant group or other permissions", errors.getvalue())

    def test_resolve_store_refuses_other_protected_paths_and_leaf_symlinks(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            codex_home = root / "codex-home"
            codex_home.mkdir()
            target = root / "target"
            target.mkdir()
            link = root / "store-link"
            link.symlink_to(target, target_is_directory=True)
            repo_root = SCRIPT.resolve().parents[1]

            for candidate in (
                Path("/"),
                Path.home(),
                repo_root,
                repo_root.parent,
                repo_root / "synthetic-store",
            ):
                with self.subTest(candidate=candidate):
                    with self.assertRaises(store_tool.StoreError):
                        store_tool.resolve_store(str(candidate), str(codex_home))
            with self.assertRaises(store_tool.StoreError):
                store_tool.resolve_store(str(link), str(codex_home))

    def test_exclusive_marker_creation_never_follows_an_existing_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            store = root / "new-store"
            store.mkdir()
            target = root / "marker-target"
            marker = store / store_tool.MARKER_NAME
            marker.symlink_to(target)

            with self.assertRaises(store_tool.StoreError):
                store_tool.create_marker_exclusive(store)

            self.assertFalse(target.exists())
            self.assertTrue(marker.is_symlink())

    def test_marker_validation_requires_exact_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            store = Path(temp) / "store"
            store.mkdir()
            marker = store / store_tool.MARKER_NAME
            marker.write_bytes((store_tool.MARKER_VALUE + "\r\n").encode())

            with self.assertRaises(store_tool.StoreError):
                store_tool.read_marker(store)

    def test_launcher_rejects_home_override_before_mutating_it(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            home.mkdir(mode=0o755)
            os.chmod(home, 0o755)
            original_mode = stat.S_IMODE(home.stat().st_mode)
            environment = os.environ.copy()
            environment.update(
                {
                    "HOME": str(home),
                    "CODEX_HOME": str(home / ".codex"),
                    "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME": str(home),
                }
            )

            result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )

            self.assertEqual(result.returncode, 2)
            self.assertIn("blocked before creating or changing", result.stderr)
            self.assertEqual(stat.S_IMODE(home.stat().st_mode), original_mode)
            self.assertFalse((home / store_tool.MARKER_NAME).exists())

    def test_launcher_rejects_leaf_symlink_before_mutating_target(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            home.mkdir()
            target = root / "target"
            target.mkdir(mode=0o755)
            os.chmod(target, 0o755)
            link = root / "store-link"
            link.symlink_to(target, target_is_directory=True)
            original_mode = stat.S_IMODE(target.stat().st_mode)
            environment = os.environ.copy()
            environment.update(
                {
                    "HOME": str(home),
                    "CODEX_HOME": str(home / ".codex"),
                    "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME": str(link),
                }
            )

            result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )

            self.assertEqual(result.returncode, 2)
            self.assertEqual(stat.S_IMODE(target.stat().st_mode), original_mode)
            self.assertFalse((target / store_tool.MARKER_NAME).exists())

    def test_launcher_refuses_a_precreated_empty_unmarked_store(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            codex_home = home / ".codex"
            store = codex_home / "flekks-medical-synthetic"
            store.mkdir(parents=True, mode=0o755)
            os.chmod(store, 0o755)
            original_mode = stat.S_IMODE(store.stat().st_mode)
            environment = os.environ.copy()
            environment.update(
                {
                    "HOME": str(home),
                    "CODEX_HOME": str(codex_home),
                    "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME": str(store),
                }
            )

            result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )

            self.assertEqual(result.returncode, 2)
            self.assertIn("preexisting store", result.stderr)
            self.assertEqual(stat.S_IMODE(store.stat().st_mode), original_mode)
            self.assertEqual(list(store.iterdir()), [])

    def test_launcher_refuses_unmarked_store_with_existing_codex_database(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            codex_home = home / ".codex"
            store = codex_home / "flekks-medical-synthetic"
            store.mkdir(parents=True, mode=0o755)
            os.chmod(store, 0o755)
            database = store / "logs_2.sqlite"
            database.write_bytes(b"do not adopt this database")
            original_mode = stat.S_IMODE(store.stat().st_mode)
            environment = os.environ.copy()
            environment.update(
                {
                    "HOME": str(home),
                    "CODEX_HOME": str(codex_home),
                    "FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME": str(store),
                }
            )

            result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )

            self.assertEqual(result.returncode, 2)
            self.assertIn("preexisting store", result.stderr)
            self.assertEqual(stat.S_IMODE(store.stat().st_mode), original_mode)
            self.assertEqual(database.read_bytes(), b"do not adopt this database")
            self.assertFalse((store / store_tool.MARKER_NAME).exists())

    def test_launcher_creates_and_unlocks_only_the_validated_default_store(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            home.mkdir()
            fake_bin = root / "bin"
            fake_bin.mkdir()
            cargo_log = root / "cargo.log"
            fake_cargo = fake_bin / "cargo"
            fake_cargo.write_text(
                '#!/bin/sh\nprintf \'%s\\n\' "$*" >"$FLEKKS_TEST_CARGO_LOG"\n',
                encoding="utf-8",
            )
            os.chmod(fake_cargo, 0o755)
            environment = os.environ.copy()
            environment.pop("FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME", None)
            environment.update(
                {
                    "HOME": str(home),
                    "CODEX_HOME": str(home / ".codex"),
                    "FLEKKS_TEST_CARGO_LOG": str(cargo_log),
                    "PATH": f"{fake_bin}{os.pathsep}{environment['PATH']}",
                }
            )

            result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )

            store = home / ".codex" / "flekks-medical-synthetic"
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            self.assertEqual(stat.S_IMODE(store.stat().st_mode), 0o700)
            self.assertEqual(
                (store / store_tool.MARKER_NAME).read_text(encoding="utf-8"),
                store_tool.MARKER_VALUE + "\n",
            )
            self.assertEqual(
                stat.S_IMODE((store / store_tool.MARKER_NAME).stat().st_mode),
                0o600,
            )
            self.assertFalse((store / store_tool.LOCK_NAME).exists())
            self.assertIn("run --profile dev-small --bin codex", cargo_log.read_text())

            second_result = subprocess.run(
                ["sh", str(LAUNCHER)],
                check=False,
                capture_output=True,
                env=environment,
                text=True,
            )
            self.assertEqual(second_result.returncode, 0, msg=second_result.stderr)
            self.assertFalse((store / store_tool.LOCK_NAME).exists())


if __name__ == "__main__":
    unittest.main()
