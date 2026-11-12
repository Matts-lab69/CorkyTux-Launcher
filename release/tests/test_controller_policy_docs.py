"""Keep the deliberately unbound host button policy explicit."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ControllerPolicyDocs(unittest.TestCase):
    def test_guide_ps_is_documented_as_unbound_without_interception(self):
        readme = (ROOT / "README.md").read_text()
        self.assertIn("Guide/PS remains unbound", readme)
        self.assertIn("Steam/Game Bar or the desktop compositor may intercept", readme)
        self.assertIn("CorkyTux does not grab this button", readme)
        source = (ROOT / "src/ui/gamepad.rs").read_text()
        mapping = source.split("fn button_input(", 1)[1].split("fn spawn_", 1)[0]
        self.assertNotIn("Button::Mode =>", mapping)


if __name__ == "__main__":
    unittest.main()
