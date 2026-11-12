"""Guard specific translated legacy UI maintenance phrases."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class UiLegacyCommentPhrases(unittest.TestCase):
    def test_specific_translated_ui_phrases_do_not_return(self):
        cases = {
            "details_panel.rs": ("Tamano 16 explicito",),
            "game_settings.rs": ("Dueno para el callback",),
            "minecraft_view.rs": ("solo respuestas limpias", "Response del verbo",
                                  "Vanilla deshabilitado", "Contrato de do_probe",
                                  "Contrato de cache", "la UI marca el build",
                                  "do_delete no es invocable", "test_env restaura HOME"),
        }
        for name, phrases in cases.items():
            source = (ROOT / "src/ui" / name).read_text()
            for phrase in phrases:
                with self.subTest(file=name, phrase=phrase):
                    self.assertNotIn(phrase, source)


if __name__ == "__main__":
    unittest.main()
