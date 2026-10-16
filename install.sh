#!/usr/bin/env bash
# Links every skill in this repository (top-level folders with a SKILL.md) into the
# skills folder of each coding agent. Existing entries that are not links to this
# repository are reported and skipped, never overwritten.
#
# Usage: ./install.sh [target_dir ...]
#   default targets: ~/.claude/skills (Claude Code) and ~/.agents/skills (cross-agent standard)
#   EXCLUDE="assistente-diario other" ./install.sh
set -euo pipefail

repo="$(cd "$(dirname "$0")" && pwd -P)"
if [ "$#" -gt 0 ]; then targets=("$@"); else targets=("$HOME/.claude/skills" "$HOME/.agents/skills"); fi
exclude=" ${EXCLUDE:-} "

for target in "${targets[@]}"; do
  echo "== $target"
  if [ -L "$target" ] && [ "$(cd "$target" && pwd -P)" = "$repo" ]; then
    echo "   already links to the whole repository, nothing to do"
    continue
  fi
  mkdir -p "$target"
  for dir in "$repo"/*/; do
    name="$(basename "$dir")"
    [ -f "$dir/SKILL.md" ] || continue
    [[ "$exclude" == *" $name "* ]] && continue
    dest="$target/$name"
    if [ -L "$dest" ] && [ "$(cd "$dest" && pwd -P)" = "$repo/$name" ]; then
      echo "   = $name"
    elif [ -e "$dest" ] || [ -L "$dest" ]; then
      echo "   ! $name: '$dest' already exists and is not a link to this repository, skipped" >&2
    else
      ln -s "$repo/$name" "$dest"
      echo "   + $name"
    fi
  done
done
