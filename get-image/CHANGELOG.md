# Changelog

## [0.2.0] - 2026-07-28

### Added
- Inline image display in the terminal after each generation, on terminals that support it (iTerm2, WezTerm, kitty, Ghostty); disable with `--no-display`
- Prompt templates: `[a|b]` groups expand into one generation per combination, e.g. "a [red|blue] [cat|dog]" generates four images (limit 16), with a total cost summary

## [0.1.0] - 2026-07-28

### Added
- Initial release
- Generate images from a text prompt via OpenRouter image models and save them to the current working directory
- Inexpensive defaults: google/gemini-2.5-flash-image, quality "low", size "512"
- Interactive session that retains the prompt: Enter regenerates, Up-arrow recalls the prompt for editing, /commands adjust model, quality, size, and count
- Browse and open generated images from the session with /list and /open
- `models` subcommand listing image-capable models with estimated price per image, cheapest first
- Provider routing sorted by price (`provider.sort = "price"`) for generation requests
- Configuration via `~/.config/cli-programs/get-image.toml` with `config show`, `config set`, and `config path` subcommands
- API key resolution from shared `llm.toml` or `OPENROUTER_API_KEY`
