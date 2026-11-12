"""Guard specific legacy fixture labels and test method vocabulary."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class RemainingLauncherTestLanguage(unittest.TestCase):
    def test_translated_fixture_and_diagnostics(self):
        cases = {
            "backend/config.rs": ('"Solo"',),
            "backend/integration.rs": ("el árbol temporal debe",),
            "ui/minecraft_view.rs": ("legacy_ver_se_borra", "builds estables", "la copia legacy debe"),
            "ui/stores_view.rs": ("granularidad_falta", "antes del skip", "tras el skip"),
            "ui/remove_modal.rs": ("HOME debe negarse", "la raíz de la biblioteca no se borra", "no debe ser borrable"),
        }
        for name, phrases in cases.items():
            source = (ROOT / "src" / name).read_text()
            for phrase in phrases:
                with self.subTest(file=name, phrase=phrase):
                    self.assertNotIn(phrase, source)
        for name in ("ui/remove_modal.rs", "ui/stores_view.rs", "backend/proton.rs"):
            source = (ROOT / "src" / name).read_text()
            self.assertIsNone(re.search(r"fn \w*(?:_se_niega|_no_visto|_nunca_cierra|granularidad_falta)\w*", source))


if __name__ == "__main__":
    unittest.main()
