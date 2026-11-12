"""Prevent reintroduction of the translated legacy Settings comment phrases."""

from pathlib import Path
import unittest


class SettingsLegacyCommentPhrases(unittest.TestCase):
    def test_specific_legacy_settings_comment_phrases_do_not_return(self):
        root = Path(__file__).resolve().parents[2]
        source = (root / "src/ui/settings.rs").read_text()
        for phrase in ("Rutas ya movidas", "Candidatos de move", "Total real movido",
                       "Import bulk con", "Destino por tienda", "Clones hechos ANTES",
                       "Registra en la biblioteca", "Nunca incrusta", "Fuera del closure",
                       "home_dir vacío", "Desde el repositorio", "Resuelve el icono"):
            with self.subTest(phrase=phrase):
                self.assertNotIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
