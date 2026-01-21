#!/bin/bash
# Update Claude Code silently on startup
npm update -g @anthropic-ai/claude-code >/dev/null 2>&1 || true
exec "$@"
