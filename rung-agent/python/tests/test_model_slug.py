import unittest

from rung_harbor.model import DEFAULT_MODEL, rung_model_slug


class SlugTests(unittest.TestCase):
    def test_default(self):
        self.assertEqual(rung_model_slug(None), DEFAULT_MODEL)
        self.assertEqual(rung_model_slug("  "), DEFAULT_MODEL)

    def test_strips_openrouter_prefix(self):
        self.assertEqual(
            rung_model_slug("openrouter/anthropic/claude-sonnet-4.5"),
            "anthropic/claude-sonnet-4.5",
        )

    def test_passthrough(self):
        self.assertEqual(
            rung_model_slug("anthropic/claude-sonnet-4.5"),
            "anthropic/claude-sonnet-4.5",
        )


if __name__ == "__main__":
    unittest.main()
