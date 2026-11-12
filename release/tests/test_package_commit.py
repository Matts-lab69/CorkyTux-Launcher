"""Packaged executables record the source commit without executing payloads."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


class PackageCommitTests(unittest.TestCase):
    def test_both_staged_executables_contain_the_source_commit(self):
        builder = Path(__file__).resolve().parents[1] / "build-release.sh"
        with tempfile.TemporaryDirectory(prefix="corkytux-package-commit-") as temp:
            root = Path(temp)
            (root / "release").mkdir()
            (root / "assets").mkdir()
            (root / "docs").mkdir()
            (root / "tools").mkdir()
            (root / "tmp").mkdir()
            shutil.copyfile(builder, root / "release/build-release.sh")
            (root / "Cargo.toml").write_text('version = "3.2.2"\n')
            for name in ["LICENSE", "README.md", "CREDITS.md", "docs/THIRD-PARTY.md",
                         "release/corkytux.png", "release/corkytux.desktop.in"]:
                (root / name).write_text("fixture\n")
            for name in ["install.sh", "uninstall.sh"]:
                (root / "release" / name).write_text("#!/bin/sh\nexit 99\n")
            # This fixture command copies an ELF file as data; it never runs it.
            cargo = root / "tools/cargo"
            cargo.write_text(
                f"#!{sys.executable}\n"
                "from pathlib import Path\nimport os, shutil\n"
                "dest = Path(os.environ['CARGO_TARGET_DIR']) / 'release'\n"
                "dest.mkdir(parents=True, exist_ok=True)\n"
                "for name in ('corkytux', 'webdriver_login'):\n"
                "    shutil.copyfile('/bin/true', dest / name)\n"
            )
            cargo.chmod(0o700)
            env = os.environ.copy()
            env.update(PATH=str(root / "tools") + os.pathsep + env["PATH"],
                       HOME=str(root), TMPDIR=str(root / "tmp"),
                       CARGO_TARGET_DIR=str(root / "target"))
            env.pop("CORKYTUX_PLUGINS_SOURCE", None)
            env.pop("CARGO_ENCODED_RUSTFLAGS", None)
            subprocess.run(["git", "init", "-q", str(root)], check=True, env=env)
            subprocess.run(["git", "add", "."], cwd=root, env=env, check=True)
            subprocess.run(["git", "-c", "user.name=PackageFixture", "-c",
                            "user.email=fixture@example.invalid", "-c",
                            "core.hooksPath=/dev/null", "commit", "-qm", "fixture"],
                           cwd=root, env=env, check=True)
            commit = subprocess.check_output(["git", "rev-parse", "HEAD"],
                                             cwd=root, env=env).strip()
            subprocess.run(["bash", "release/build-release.sh", "v3.2.2"],
                           cwd=root, env=env, check=True,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            archive = next(root.glob("corkytux-3.2.2-linux-*.tar.gz"))
            with tarfile.open(archive) as package:
                for name in ["corkytux", "webdriver_login"]:
                    with self.subTest(binary=name):
                        data = package.extractfile("corkytux-3.2.2/" + name).read()
                        self.assertTrue(commit in data, "source commit metadata is missing")
