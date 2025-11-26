# Changelog

## [0.1.0] - 2025-11-25

### Added
- Initial release of llm-client shared library
- LlmProvider trait for unified LLM interface
- Claude CLI provider (subprocess-based, default)
- Anthropic API provider via genai crate
- OpenRouter provider for multi-model access
- Cerebras provider for fast Llama inference
- Configuration system with TOML file support
- Model presets for quick provider/model selection
