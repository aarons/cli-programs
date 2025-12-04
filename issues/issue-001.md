# Refactor Sandy Config System for Testability

The sandy config and state system uses hardcoded paths derived from `$HOME`, making unit tests dependent on the real user's config directory. This causes test failures when the user has existing config files from actual usage.

## Context

### Current Behavior
- `Config::config_dir()` returns `~/.config/cli-programs` with no way to override
- Functions like `check_default_template_status()` mix I/O (reading stored hashes from disk) with business logic (comparing hashes to determine status)
- Unit tests that call these functions read from the real config directory
- If the user has used `sandy` before, tests may fail because they encounter real config state instead of the expected empty/default state

### Example Failure
The test `test_check_default_template_status_up_to_date_matches_current` assumes no stored hash exists, but on a machine with prior `sandy` usage, the file `~/.config/cli-programs/sandy-default-template.hash` exists and contains a real hash, causing the test to fail.

### Desired Behavior
- Config directory path should be injectable/overridable for testing
- Functions should separate I/O concerns from business logic so the logic can be unit tested in isolation
- Unit tests should not depend on or affect the user's real config directory

## Implementation Notes

### Relevant Files
- `sandy/src/config.rs` - `Config::config_dir()` returns hardcoded path (line 67-70)
- `sandy/src/state.rs` - Hash loading/saving functions use `Config::config_dir()` (lines 88-144)
- `sandy/src/docker.rs` - `check_default_template_status()` mixes I/O with logic (lines 114-159)

### Current Architecture
```
Config::config_dir() -> PathBuf  (hardcoded to ~/.config/cli-programs)
    |
    +-> sandy.toml (config file)
    +-> sandy-state.json (sandbox tracking)
    +-> sandy-template.hash (built image hash)
    +-> sandy-default-template.hash (tracks which embedded default was used)
```

### Integration Tests vs Unit Tests
The CLI integration tests in `sandy/tests/cli.rs` correctly handle this by overriding `HOME` environment variable:
```rust
sandy_cmd()
    .arg("list")
    .env("HOME", temp_dir.path())  // Redirects config dir to temp
    ...
```

However, unit tests run in-process and cannot easily override environment variables without affecting parallel tests.

## Suggested Approach

### 1. Make Config Directory Injectable
Add a way to override the config directory, either via:
- An environment variable (e.g., `SANDY_CONFIG_DIR`)
- A builder pattern on `Config`
- A thread-local override for testing

### 2. Separate I/O from Logic
Refactor functions like `check_default_template_status` to accept the stored hash as a parameter rather than reading it internally:

```rust
// Current: mixes I/O with logic
pub fn check_default_template_status(
    dockerfile_path: &Path,
    default_template: &str,
) -> Result<DefaultTemplateStatus> {
    let stored_default_hash = load_default_template_hash()?;  // I/O here
    // ... logic using stored_default_hash
}

// Proposed: separate concerns
pub fn check_default_template_status(
    dockerfile_path: &Path,
    default_template: &str,
) -> Result<DefaultTemplateStatus> {
    let stored_hash = load_default_template_hash()?;
    check_default_template_status_impl(dockerfile_path, default_template, stored_hash)
}

// Pure logic, easily testable
fn check_default_template_status_impl(
    dockerfile_path: &Path,
    default_template: &str,
    stored_default_hash: Option<String>,
) -> Result<DefaultTemplateStatus> {
    // ... pure logic
}
```

### 3. Update Tests
- Unit tests call the pure logic functions with explicit parameters
- Integration tests continue using HOME override for end-to-end testing

## Testing & Validation

### Unit Tests Should Cover
- All branches of `check_default_template_status` logic:
  - No dockerfile exists -> `NeedsCreation`
  - No stored hash + file matches default -> `UpToDate`
  - No stored hash + file differs from default -> `Customized`
  - Stored hash exists + file matches stored hash + stored != current default -> `NeedsUpdate`
  - Stored hash exists + file matches stored hash + stored == current default -> `UpToDate`
  - Stored hash exists + file differs from stored hash -> `Customized`

### Validation Steps
1. All existing tests should pass
2. Tests should pass on a clean machine (no config)
3. Tests should pass on a machine with existing sandy config
4. Run `cargo test -p sandy` multiple times to verify no flaky tests

## Documentation

- No external documentation changes needed
- Code comments should explain the separation of concerns pattern for future contributors
