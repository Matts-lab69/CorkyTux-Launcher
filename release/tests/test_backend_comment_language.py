"""Guard against reintroducing the specifically translated legacy phrases."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class BackendCommentLanguage(unittest.TestCase):
    def test_translated_legacy_backend_comment_phrases_do_not_return(self):
        cases = {
            "proton.rs": ("Paridad con", "Aislamiento Bottles", "del usuario actual",
                          "Las bibliotecas", "Soporte 32/64", "Recorte del catalogo",
                          "C++ parity: los tools", "Fase 4", "mismo release (verificado"),
            "integration.rs": ("lee la config", "tomo el lock", "Idempotente sobre"),
        }
        for name, phrases in cases.items():
            source = (ROOT / "src/backend" / name).read_text()
            for phrase in phrases:
                with self.subTest(file=name, phrase=phrase):
                    self.assertNotIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
