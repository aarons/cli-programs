# sandbox - Future Improvements

## High Priority

- [ ] **Add `--mount-docker-socket` flag** - Enable container-building tasks inside sandboxes by optionally mounting `/var/run/docker.sock`

- [ ] **Improve error handling for missing Docker sandbox extension** - Provide clearer installation instructions and detect if Docker Desktop is running

- [ ] **Add `sandbox exec <name> <command>` subcommand** - Run arbitrary commands in a running sandbox without attaching

- [ ] **Handle orphaned state entries** - Detect and clean up state entries for worktrees that no longer exist on disk

## Medium Priority

- [ ] **Add `--detach` flag to `sandbox new`** - Start sandbox in background without attaching

- [ ] **Support multiple prompts/tasks per sandbox** - Allow passing an initial prompt to Claude when starting

- [ ] **Add `sandbox logs <name>`** - View container logs without attaching

- [ ] **Improve interactive selection** - Use arrow key navigation instead of numbered selection (consider `dialoguer` crate)

- [ ] **Add worktree branch tracking** - Show current branch in `sandbox list`, detect if worktree has uncommitted changes

## Lower Priority

- [ ] **Add `sandbox stop <name>`** - Stop a running sandbox without removing it

- [ ] **Add `sandbox prune`** - Remove all stopped containers and optionally orphaned worktrees

- [ ] **Template variants** - Support multiple named Dockerfile templates for different project types

- [ ] **Config validation** - Warn about invalid mount paths or missing environment variables at startup

- [ ] **Shell completions** - Generate completions for bash/zsh/fish

## Considerations

- The `docker sandbox` command is relatively new - monitor for API changes
- Worktree names must be unique across all repositories (consider namespacing by repo)
- Container naming uses path hash - changing worktree location breaks association
