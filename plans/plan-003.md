# Add tests for LLM retry and fallback logic

Add meaningful test coverage for the retry-with-exponential-backoff and fallback behavior in the `gc` program's LLM client.

## Context

### Business Case

The `gc` program generates git commit messages using LLM providers. Recently, we added resilience features to handle API overload (HTTP 503) errors:

1. **Retry with exponential backoff**: When a 503 "server overloaded" error occurs, retry up to 3 times with 1s, 2s, 4s delays
2. **Fallback to claude-cli**: If retries fail and the user configured a different provider (e.g., Anthropic API), fall back to the Claude CLI provider

This logic is critical for user experience - without it, users see cryptic errors during high-traffic periods. However, there are **zero tests** covering this behavior.

### Current vs Desired State

**Current**: The retry/fallback logic in `gc/src/llm.rs` has no test coverage. If someone modifies this code, they could easily break:
- The exponential backoff timing
- The condition for triggering fallback
- The handling of non-retryable errors

**Desired**: Tests that document and verify the core behaviors, helping future engineers understand the intended design.

### Related Code Changes

The retry/fallback feature was implemented across:
- `llm-client/src/error.rs` - Added `ServerOverloaded` error variant
- `llm-client/src/providers/anthropic.rs` - Detects 503 and returns `ServerOverloaded`
- `llm-client/src/providers/openai_compatible.rs` - Same 503 detection
- `gc/src/llm.rs` - Retry loop, backoff calculation, fallback logic

## Implementation Notes

### Architecture

The `gc` program uses a layered architecture:

```
gc/src/main.rs          # CLI entry point, git operations
    └── gc/src/llm.rs   # LlmClient wrapper (retry/fallback logic lives here)
            └── llm-client/  # Shared library with provider implementations
                    ├── src/provider.rs      # LlmProvider trait
                    ├── src/error.rs         # LlmError enum
                    └── src/providers/       # Anthropic, ClaudeCli, OpenAI-compatible
```

### Current LlmClient Design

```rust
// gc/src/llm.rs
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    config: Config,
    preset_name: String,
    debug: bool,
}

impl LlmClient {
    pub fn new(preset_name: Option<&str>, debug: bool) -> Result<Self> {
        // Loads config from disk, creates real provider
        // This is hard to test because we can't inject a mock
    }

    pub async fn complete(&self, prompt: &str, system_prompt: &str) -> Result<String> {
        // Retry logic with exponential backoff
        // Fallback to claude-cli if retries exhausted
    }
}
```

### The Testing Challenge

The `LlmClient::new()` constructor loads config from disk and creates real providers, making it impossible to test the retry logic in isolation. We need a way to inject a controllable provider.

### Key Trait Definition

```rust
// llm-client/src/provider.rs
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;
    fn name(&self) -> &'static str;
    fn is_available(&self) -> Result<()>;
}
```

## Suggested Approach

### 1. Add test constructor to LlmClient

Add a `#[cfg(test)]` constructor that accepts an injected provider:

```rust
// gc/src/llm.rs
impl LlmClient {
    #[cfg(test)]
    pub fn with_provider(
        provider: Box<dyn LlmProvider>,
        config: Config,
        preset_name: String,
    ) -> Self {
        Self {
            provider,
            config,
            preset_name,
            debug: false,
        }
    }
}
```

### 2. Create MockProvider in llm-client

Add a configurable mock provider for testing:

```rust
// llm-client/src/providers/mock.rs (new file, only compiled in test)
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct MockProvider {
    fail_count: AtomicUsize,
    fail_with: Option<LlmError>,
    success_response: String,
}

impl MockProvider {
    /// Create a provider that fails `n` times then succeeds
    pub fn fails_then_succeeds(n: usize, error: LlmError, response: &str) -> Self { ... }

    /// Create a provider that always fails
    pub fn always_fails(error: LlmError) -> Self { ... }

    /// Create a provider that always succeeds
    pub fn always_succeeds(response: &str) -> Self { ... }

    /// Get the number of times complete() was called
    pub fn call_count(&self) -> usize { ... }
}
```

### 3. Write focused tests

```rust
// gc/src/llm.rs (in #[cfg(test)] mod tests)

#[tokio::test]
async fn retries_on_server_overloaded() {
    // Provider fails twice with 503, then succeeds
    // Verify: returns success, provider called exactly 3 times
}

#[tokio::test]
async fn no_retry_on_other_errors() {
    // Provider fails with MissingApiKey
    // Verify: fails immediately, provider called only once
}

#[tokio::test]
async fn fallback_triggered_after_retries_exhausted() {
    // Provider always fails with 503
    // Config has claude-cli fallback available
    // Verify: attempts fallback after 3 retries
}

#[tokio::test]
async fn no_fallback_when_already_using_claude_cli() {
    // preset_name is "claude-cli", provider always fails with 503
    // Verify: fails after 3 retries, no fallback attempted
}
```

## Testing

Avoid introducing boilerplate tests; we do not want excessive pointless tests as these do not serve anyone.
It's extremely important that the tests are meaningful, clear, and validate core issues and behavior.
It's important to figure out tests that validate our business case, and that ensure healthy core architecture.
They can and should help engineers understand the intention behind the code.

### What NOT to test

- Error message formatting (boilerplate)
- Backoff timing precision (implementation detail)
- Config serialization (already tested elsewhere)

### What TO test

- Retry count: exactly 3 attempts on ServerOverloaded
- Immediate failure: non-retryable errors don't trigger retry
- Fallback conditions: only when using non-default provider AND retries exhausted
- No infinite fallback: when already using claude-cli, don't attempt fallback

## Validation

- [ ] `cargo test -p gc` passes with new tests
- [ ] `cargo test -p llm-client` passes (if mock provider added there)
- [ ] Tests clearly document the intended behavior (readable test names and assertions)
- [ ] No boilerplate tests that just check trivial things
- [ ] Coverage of the four core behaviors listed above

## Documentation

No documentation updates required. The tests themselves serve as documentation of the intended behavior.
