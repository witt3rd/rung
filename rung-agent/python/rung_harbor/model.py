"""Harbor `-m` → OpenRouter model id. No Harbor import."""

DEFAULT_MODEL = "anthropic/claude-sonnet-4.5"


def rung_model_slug(model_name: str | None) -> str:
    """Strip a leading `openrouter/` so OpenRouter sees `provider/model`."""
    if not model_name or not model_name.strip():
        return DEFAULT_MODEL
    name = model_name.strip()
    if name.startswith("openrouter/"):
        rest = name[len("openrouter/") :].strip()
        return rest or DEFAULT_MODEL
    return name
