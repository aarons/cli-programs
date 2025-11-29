#!/bin/bash
# Setup script for ask shell integration
# Adds a wrapper function to handle special characters without quoting

set -e

# Zsh: Use alias because noglob must be a precommand modifier.
# With a function, glob expansion happens BEFORE the function is called.
# With an alias, noglob is applied to the entire command line.
ZSH_FUNCTION='alias ask='\''noglob command ask'\'''

BASH_FUNCTION='ask() {
  set -f
  command ask "$@"
  local ret=$?
  set +f
  return $ret
}'

# Detect shell
SHELL_NAME=$(basename "$SHELL")

case "$SHELL_NAME" in
  zsh)
    RC_FILE="$HOME/.zshrc"
    FUNCTION="$ZSH_FUNCTION"
    ;;
  bash)
    RC_FILE="$HOME/.bashrc"
    FUNCTION="$BASH_FUNCTION"
    ;;
  *)
    echo "Unsupported shell: $SHELL_NAME"
    echo "Supported shells: zsh, bash"
    exit 1
    ;;
esac

echo "Detected shell: $SHELL_NAME"
echo "Config file: $RC_FILE"
echo ""
echo "This will add the following to $RC_FILE:"
echo ""
echo "$FUNCTION"
echo ""

# Check if already installed (function or alias)
if grep -q "^ask()\|^alias ask=" "$RC_FILE" 2>/dev/null; then
  echo "An 'ask' function or alias already exists in $RC_FILE"
  echo "Please remove it manually if you want to reinstall."
  exit 1
fi

read -p "Add this function to $RC_FILE? [y/N] " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
  echo "" >> "$RC_FILE"
  echo "# ask shell integration - handle special characters without quoting" >> "$RC_FILE"
  echo "$FUNCTION" >> "$RC_FILE"
  echo ""
  echo "Done! Run this to activate:"
  echo "  source $RC_FILE"
  echo ""
  echo "Or restart your terminal."
else
  echo "Aborted. You can manually add the function to your shell config."
fi
