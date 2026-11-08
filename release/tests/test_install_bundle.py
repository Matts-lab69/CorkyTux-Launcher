"""Installer lifecycle tests; every write stays below a temporary HOME."""
import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

RELEASE = Path(__file__).resolve().parents[1]
APP_ID = re.search(r'\.application_id\("([^"]+)"\)', (RELEASE.parent / 'src/main.rs').read_text()).group(1)


@unittest.skipIf(os.geteuid() == 0, "installer deliberately refuses root")
class BundledPluginInstall(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="corky-bundle-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home with spaces"
        self.home.mkdir()
        self.package = self.root / "corkytux-3.2.0"
        self.package.mkdir()
        for script in ["install.sh", "uninstall.sh"]:
            shutil.copy2(RELEASE / script, self.package / script)
        for binary in ["corkytux", "webdriver_login"]:
            (self.package / binary).write_bytes(b"\x7fELF installer-test fixture\n")
        (self.package / "assets").mkdir()
        (self.package / "assets/icon.txt").write_text("fixture\n")
        shutil.copy2(RELEASE / 'corkytux.png', self.package / 'corkytux.png')
        self.env = dict(os.environ, HOME=str(self.home), XDG_DATA_HOME="")
        self.install_dir = self.home / ".local/share/corkytux"
        self.plugin_root = self.home / ".local/share/CorkyTux/plugins"
        self.source = self.package / "plugins/sample-plugin"
        self.source.mkdir(parents=True)
        (self.source / "plugin.json").write_text(json.dumps({"id": "sample-plugin"}))
        (self.source / "sample-plugin").write_text("#!/bin/sh\nexit 0\n")

    def run_script(self, script, *args, answer="n\nn\nn\n", success=True):
        result = subprocess.run(
            ["bash", str(self.package / script), *args], input=answer,
            text=True, capture_output=True, env=self.env, timeout=60,
        )
        self.assertTrue((result.returncode == 0) == success, "unexpected script result")
        return result

    def test_install_reinstall_and_uninstall_preserve_untracked_state(self):
        self.run_script("install.sh", answer="y\n")
        entry = self.plugin_root / "sample-plugin/sample-plugin"
        self.assertTrue(entry.is_file())
        self.assertTrue(os.access(entry, os.X_OK))
        runtime = entry.parent / "runtime/state.txt"
        runtime.parent.mkdir()
        runtime.write_text("runtime stays\n")
        other = self.plugin_root / "unrelated/entry"
        other.parent.mkdir()
        other.write_text("unrelated stays\n")
        (self.source / "sample-plugin").write_text("#!/bin/sh\nexit 1\n")
        self.run_script("install.sh", answer="y\n")
        self.assertEqual(entry.read_bytes(), (self.source / "sample-plugin").read_bytes())
        self.assertTrue(runtime.is_file())
        self.run_script("uninstall.sh")
        self.assertFalse(entry.exists())
        self.assertFalse((entry.parent / "plugin.json").exists())
        self.assertTrue(runtime.is_file())
        self.assertTrue(other.is_file())
        self.assertFalse(self.install_dir.exists())

    def test_no_plugins_skips_bundled_code(self):
        self.run_script("install.sh", "--no-plugins", answer="y\n")
        self.assertFalse(self.plugin_root.exists())
        self.assertFalse((self.install_dir / "plugins-installed.txt").exists())
        self.assertTrue((self.install_dir / "corkytux").exists())
        self.run_script("uninstall.sh")

    def test_no_plugins_on_reinstall_keeps_previous_tracking(self):
        self.run_script("install.sh", answer="y\n")
        self.run_script("install.sh", "--no-plugins", answer="y\n")
        self.assertTrue((self.plugin_root / "sample-plugin/sample-plugin").exists())
        self.run_script("uninstall.sh")
        self.assertFalse((self.plugin_root / "sample-plugin/sample-plugin").exists())

    def test_absolute_xdg_data_home_matches_launcher(self):
        xdg = self.root / "xdg data with spaces"
        self.env["XDG_DATA_HOME"] = str(xdg)
        self.run_script("install.sh", answer="y\n")
        entry = xdg / "CorkyTux/plugins/sample-plugin/sample-plugin"
        self.assertTrue(entry.is_file())
        self.assertFalse(self.plugin_root.exists())
        self.assertTrue((xdg / f"applications/{APP_ID}.desktop").is_file())
        self.assertTrue((xdg / f"icons/hicolor/128x128/apps/{APP_ID}.png").is_file())
        self.run_script("uninstall.sh")
        self.assertFalse(entry.exists())
        self.assertFalse((xdg / f"applications/{APP_ID}.desktop").exists())

    def test_relative_xdg_data_home_falls_back_to_home(self):
        self.env["XDG_DATA_HOME"] = "relative-not-used"
        self.run_script("install.sh", answer="y\n")
        self.assertTrue((self.plugin_root / "sample-plugin/sample-plugin").exists())
        self.run_script("uninstall.sh")

    def test_missing_python_modules_recommends_managed_install(self):
        fake_bin = self.root / "fake-bin"
        fake_bin.mkdir()
        fake_python = fake_bin / "python3"
        fake_python.write_text("#!/bin/sh\nexit 1\n")
        fake_python.chmod(0o755)
        self.env["PATH"] = f"{fake_bin}:{os.environ['PATH']}"

        result = self.run_script("install.sh", answer="y\n")
        self.assertIn("Missing python modules: requests", result.stdout)
        self.assertIn("virtual environment with its bin directory on PATH", result.stdout)
        self.assertNotIn("pip install --user", result.stdout)

    def test_symlink_payload_is_refused_before_installing_binary(self):
        (self.source / "link").symlink_to(self.root / "outside")
        self.run_script("install.sh", success=False)
        self.assertFalse(self.install_dir.exists())

    def test_symlink_destination_is_refused_without_modifying_target(self):
        outside = self.root / "outside"
        outside.mkdir()
        self.plugin_root.mkdir(parents=True)
        (self.plugin_root / "sample-plugin").symlink_to(outside, target_is_directory=True)
        self.run_script("install.sh", success=False)
        self.assertEqual(list(outside.iterdir()), [])
        self.assertFalse(self.install_dir.exists())

    def test_uninstall_refuses_manifest_traversal(self):
        self.run_script("install.sh", answer="y\n")
        outside = self.root / "outside.txt"
        outside.write_text("preserve\n")
        (self.install_dir / "plugins-installed.txt").write_text("../../outside.txt\n")
        self.run_script("uninstall.sh", success=False)
        self.assertEqual(outside.read_text(), "preserve\n")

    def test_desktop_and_hicolor_match_the_gtk_application_id(self):
        self.run_script("install.sh", answer="y\n")
        data = self.home / '.local/share'
        desktop = data / f'applications/{APP_ID}.desktop'
        fields = dict(line.split('=', 1) for line in desktop.read_text().splitlines() if '=' in line)
        self.assertEqual(fields['Icon'], APP_ID)
        self.assertEqual(fields['StartupWMClass'], APP_ID)
        self.assertEqual((data / f'icons/hicolor/128x128/apps/{APP_ID}.png').read_bytes(),
                         (self.package / 'corkytux.png').read_bytes())
        self.run_script('uninstall.sh')
        self.assertFalse(desktop.exists())
        self.assertFalse((data / f'icons/hicolor/128x128/apps/{APP_ID}.png').exists())

    def test_reinstall_migrates_only_our_legacy_desktop_entry(self):
        self.run_script('install.sh', answer='y\n')
        applications = self.home / '.local/share/applications'
        canonical = applications / f'{APP_ID}.desktop'
        legacy = applications / 'corkytux.desktop'
        shutil.copy2(canonical, legacy)
        self.run_script('install.sh', answer='y\n')
        self.assertTrue(canonical.is_file())
        self.assertFalse(legacy.exists())
        self.assertTrue(list(applications.glob('corkytux.desktop.bak-*')))
        legacy.write_text('[Desktop Entry]\nExec=/unrelated/application\n')
        self.run_script('install.sh', answer='y\n')
        self.assertTrue(legacy.is_file(), 'unrelated shortcut removed')

    def test_missing_icon_is_rejected_before_installing(self):
        (self.package / 'corkytux.png').unlink()
        self.run_script('install.sh', success=False)
        self.assertFalse(self.install_dir.exists())


if __name__ == "__main__":
    unittest.main()
