# gc-updates Branch: Refactoring TODO

## High Priority

### 1. Restore empty repo fallback (BUG FIX)
The fallback for empty repos was removed in `get_branch_commits()`. This breaks initial commits.

**Location:** `gc/src/main.rs:205-208`

**Fix:** Restore the match pattern that returns `"Initial commit"` when git log fails.

### 2. Replace `genai` with direct `reqwest` calls
The `genai` crate adds ~1300 lines to Cargo.lock. For simple completions, this is unnecessary overhead.

**Scope:**
- Remove `genai` dependency from `llm-client/Cargo.toml`
- Add `reqwest` with minimal features (`json`, `rustls-tls`)
- Rewrite `AnthropicProvider`, `OpenRouterProvider`, `CerebrasProvider` to use direct HTTP calls
- Define simple request/response structs for the OpenAI-compatible chat API

## Medium Priority

### 3. Consolidate OpenAI-compatible providers
`cerebras.rs` and `openrouter.rs` are ~90% identical. Create a generic provider.

**New structure:**
```rust
// providers/openai_compatible.rs
pub struct OpenAICompatibleProvider {
    model: String,
    base_url: String,
    api_key: String,
    name: &'static str,
}

impl OpenAICompatibleProvider {
    pub fn cerebras(model: &str, api_key: String) -> Result<Self> {
        Self::new(model, api_key, "https://api.cerebras.ai/v1/", "Cerebras")
    }

    pub fn openrouter(model: &str, api_key: String) -> Result<Self> {
        Self::new(model, api_key, "https://openrouter.ai/api/v1/", "OpenRouter")
    }
}
```

**Files to modify:**
- Delete `llm-client/src/providers/cerebras.rs`
- Delete `llm-client/src/providers/openrouter.rs`
- Create `llm-client/src/providers/openai_compatible.rs`
- Update `llm-client/src/providers/mod.rs`

### 4. Move Claude CLI availability check into constructor
Currently `is_available()` is called after provider creation, but for API providers it's a no-op.

**Changes:**
- `ClaudeCliProvider::new()` should check if CLI exists and return error if not
- Remove or simplify `is_available()` trait method
- Remove the post-construction check in `gc/src/llm.rs:32-35`

### 5. Remove unused temperature/max_tokens from ModelPreset
These fields exist but are never passed to providers.

**Option A:** Remove the fields entirely
**Option B:** Actually wire them through to providers

Recommendation: Option A (remove), add back when actually needed.

## Low Priority

### 6. Use Debug formatting instead of toml in `config show`
Removes `toml` as a direct dependency of `gc` (it's still in `llm-client`).

**Location:** `gc/src/main.rs` in `handle_config_command()` for `ConfigAction::Show`

**Change:** Replace `toml::to_string_pretty(&config)` with `{:#?}` debug formatting.

### 7. Config path naming - needs decision
Currently hardcoded to `~/.config/gc/config.toml` which:
- Couples `llm-client` to `gc` specifically
- May conflict if other tools want to use `llm-client`

**Options to consider:**
1. `~/.config/llm-client/config.toml` - generic but risk of conflict with other projects
2. `~/.config/cli-programs/llm.toml` - workspace-specific, low conflict risk
3. `~/.config/aaron-cli/llm.toml` - personal namespace, very low conflict risk
4. Pass config path as parameter to `Config::load()` - most flexible, each tool decides

**Recommendation:** Option 4 (parameterize) with a default of option 2 or 3. This gives flexibility while having sensible defaults. The `cli-programs` name matches the workspace and is unlikely to conflict.

**Implementation:**
```rust
impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from(Self::default_path()?)
    }

    pub fn load_from(path: PathBuf) -> Result<Self> {
        // ... existing logic
    }

    pub fn default_path() -> Result<PathBuf> {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home).join(".config/cli-programs/llm.toml"))
    }
}
```

---

## Checklist

- [x] Restore empty repo fallback
- [x] Replace genai with reqwest
- [x] Consolidate OpenAI-compatible providers
- [x] Move CLI check to constructor
- [x] Remove unused preset fields
- [x] Use Debug formatting for config show
- [x] Decide and implement config path strategy
