"""Guard the specifically translated residual Stores identifiers/comments."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class ResidualStoreLanguage(unittest.TestCase):
    def test_specific_legacy_store_identifiers_are_absent(self):
        source = (ROOT / "src/ui/stores_view.rs").read_text()
        for name in ("bloqueado", "filas", "padre", "salida", "nombre", "pendientes",
                     "sin_browser", "titulo", "texto", "ficha", "anio", "juegos",
                     "filtrar_ordenar", "pendiente", "tras_skip", "completo", "antes",
                     "despues", "lleno", "minimo"):
            with self.subTest(identifier=name):
                self.assertIsNone(re.search(rf"\b{name}\b", source))
        for phrase in ("Última lista", "Tarjetas compactas", "Login automatizado",
                       "Scrollbar siempre", "estable aunque", "cualquier otro", "Vive en un hilo"):
            self.assertNotIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
