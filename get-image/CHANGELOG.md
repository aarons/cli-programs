# Changelog

## [0.2.0] - 2026-08-20

### Added
- `--reference`/`-r` flag (repeatable) to pass reference images for image-to-image generation, as local files (embedded as base64 data URLs) or HTTP(S) URLs
- `/reference` session command to add, list, and clear reference images
- Reference images are checked against the model's capabilities before sending: models without `input_references` support, or with a lower maximum than the number given, are refused with a clear message instead of a paid failed request

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
