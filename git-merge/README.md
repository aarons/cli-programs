# git-merge - Automated Git Branch Merging

Streamlines the process of merging feature branches into main with support for both simple and squash merges.

## Example

### Simple Merge (default)

When you run `git-merge` from a feature branch, it performs a standard merge:

```bash
$ git-merge
```

This will:
1. Push the current branch to origin
2. Switch to main
3. Pull latest changes from origin
4. Merge the feature branch into main
5. Push main to origin
6. Delete the local feature branch (if merge successful)

### Squash Merge

For squash merging with AI-generated commit messages:

```bash
$ git-merge --squash
```

This will:
1. Push the current branch to origin
2. Switch to main
3. Pull latest changes from origin
4. Squash merge the feature branch
5. Use `gc` to generate a commit message based on branch history
6. Push main to origin
7. Delete the local feature branch

## CLI Flags

- `--squash`, `-s` - Perform a squash merge instead of a regular merge
- `--target-branch <NAME>`, `-t` - Specify target branch name (default: "main")
- `<BRANCH>` - Feature branch to merge (defaults to current branch)

## Usage

### Merge current branch to main
```bash
git-merge
```
Uses current branch and performs a simple merge.

### Merge specific branch
```bash
git-merge feature/new-login
```
Merges the specified feature branch into main.

### Squash merge current branch
```bash
git-merge --squash
```
Squash merges current branch, generates AI commit message using `gc`.

### Squash merge with custom target branch
```bash
git-merge --squash --target-branch develop
```
Squash merges into 'develop' instead of 'main'.

## Requirements

- Git must be installed and repository initialized
- For squash merges: `gc` must be available in PATH (install from this workspace)
- Current branch must not be the main branch (unless specifying branch explicitly)

### Error Handling

- Validates git is installed before proceeding
- Checks for clean working tree after pulling main
- Detects merge conflicts and provides clear error messages
- Handles missing `gc` gracefully for squash merges
- Warns if branch deletion fails but merge succeeded

## Build

```bash
# Build git-merge specifically
cargo build -p git-merge --release

# The binary will be at target/release/git-merge
```

## Installation

Install to ~/.local/bin using the workspace installer:

```bash
cargo run -p update-cli-programs --release
```

## Testing

```bash
# Build and test
cargo build -p git-merge
cargo test -p git-merge

# Test in a real repository
./target/debug/git-merge --help
```
