# Issue 001: Improve sandbox naming for readability

## Summary

Change sandbox container naming from hash-based to directory-based names for easier identification.

## Current Behavior

Sandboxes are named using a hash of the workspace path:

```
sandbox-aa291d882c9c   /Users/aaron/code/sts/picky-relics
sandbox-e1664bd8231b   /Users/aaron/code/cli-programs
```

## Proposed Behavior

Name sandboxes after the directory name:

```
sandbox-picky-relics   /Users/aaron/code/sts/picky-relics
sandbox-cli-programs   /Users/aaron/code/cli-programs
```

## Implementation Notes

The naming logic is in `sandbox/src/docker.rs:20-26`:

```rust
fn get_container_name(workspace: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace.to_string_lossy().as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!("sandbox-{}", &hash[..12])
}
```

### Considerations

1. **Collision handling**: Two repos with the same directory name (e.g., `/code/foo` and `/projects/foo`) would collide. Options:
   - Append a short hash suffix: `sandbox-foo-a1b2c3`
   - Check for existing container with same name and add suffix if needed
   - Use parent directory: `sandbox-code-foo`

2. **Character sanitization**: Docker container names must match `[a-zA-Z0-9][a-zA-Z0-9_.-]*`. Directory names with spaces or special characters need sanitization.

3. **Migration**: Existing sandboxes use hash-based names. Consider:
   - Just let old sandboxes keep their names (they'll be cleaned up eventually)
   - Add a migration that renames existing containers

### Suggested Approach

Use `{dirname}-{short_hash}` format for uniqueness with readability:

```rust
fn get_container_name(workspace: &Path) -> String {
    let dirname = workspace
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| "sandbox".into());

    // Sanitize for Docker container name requirements
    let sanitized: String = dirname
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();

    // Add short hash for uniqueness
    let mut hasher = Sha256::new();
    hasher.update(workspace.to_string_lossy().as_bytes());
    let hash = hex::encode(hasher.finalize());

    format!("sandbox-{}-{}", sanitized.to_lowercase(), &hash[..6])
}
```

Result: `sandbox-cli-programs-e1664b`

## Testing

1. Verify new sandboxes get readable names
2. Verify two repos with same dirname get unique names
3. Verify special characters in directory names are handled
4. Verify existing hash-named sandboxes still work (state file stores path, not name)
