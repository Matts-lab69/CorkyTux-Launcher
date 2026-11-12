"""Guard the dependency modal's translated identifiers and locale contract."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DependencyModalLanguage(unittest.TestCase):
    def test_legacy_identifiers_and_inaccurate_plugin_claim_are_absent(self):
        source = (ROOT / "src/ui/deps_modal.rs").read_text()
        for name in ("Avance", "FilaEstado", "FilaWidgets", "TodoOk", "motivo_corto",
                     "mostrar_skip", "asegurar_css_ready", "instalar", "nombre",
                     "descripcion", "peso", "pendientes", "terminado", "estado"):
            with self.subTest(identifier=name):
                self.assertIsNone(re.search(rf"\b{name}\b", source))
        self.assertNotIn("heroic-store and the RPG runtime still answer in", source)
        self.assertIn("external tools in a Spanish locale", source)
        # These compatibility inputs must remain supported, not be translated.
        for phrase in ("red al conectar", "conexi", "cortó", "sin espacio"):
            self.assertIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
