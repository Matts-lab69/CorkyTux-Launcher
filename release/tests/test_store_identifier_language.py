"""Require English names in the Stores dependency precheck contract."""

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class StoreIdentifierLanguage(unittest.TestCase):
    def test_dependency_precheck_identifiers_are_english(self):
        source = (ROOT / "src/ui/stores_view.rs").read_text()
        for identifier in ("Chequeo", "Faltantes", "TodoBien", "Falta",
                           "faltantes_actuales", "precheck_actual", "hay_algo",
                           "aplicar_faltantes", "para_tienda", "tab_bloqueado",
                           "login_bloqueado", "filas_modal"):
            with self.subTest(identifier=identifier):
                self.assertIsNone(re.search(rf"\b{identifier}\b", source))


if __name__ == "__main__":
    unittest.main()
