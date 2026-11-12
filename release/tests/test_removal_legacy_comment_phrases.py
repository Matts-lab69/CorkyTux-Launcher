"""Guard specific translated legacy removal-safety comment phrases."""

from pathlib import Path
import unittest


class RemovalLegacyCommentPhrases(unittest.TestCase):
    def test_specific_translated_removal_phrases_do_not_return(self):
        source = (Path(__file__).resolve().parents[2] / "src/ui/remove_modal.rs").read_text()
        for phrase in ("Dirs de usuario", "Raíces de biblioteca", "Un symlink como",
                       "Rutas demasiado", "Decisión pura", "Sin registro legible",
                       "HOME sí está", "Árbol sintético", "Guard nuevo",
                       "La decisión del else", "Guard: la carpeta"):
            with self.subTest(phrase=phrase):
                self.assertNotIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
