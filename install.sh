#!/usr/bin/env bash
# Links every skill in a source folder (top-level folders with a SKILL.md) into the
# skills folder of each coding agent. Existing entries that are not links to the
# source are reported and skipped, never overwritten.
#
# Usage: ./install.sh [target_dir ...]
#   default targets: ~/.claude/skills (Claude Code), ~/.agents/skills (cross-agent standard)
#                    and ~/.gemini/config/skills (Gemini)
#   SOURCE=~/.ai-code-agent-skills-personal ./install.sh
#   EXCLUDE="skill-a skill-b" ./install.sh
set -euo pipefail

repo="$(cd "${SOURCE:-$(dirname "$0")}" && pwd -P)"
if [ "$#" -gt 0 ]; then
  targets=("$@")
else
  targets=("$HOME/.claude/skills" "$HOME/.agents/skills" "$HOME/.gemini/config/skills")
fi
exclude=" ${EXCLUDE:-} "

for target in "${targets[@]}"; do
  echo "== $target"
  if [ -L "$target" ]; then
    echo "   '$target' is itself a link; replace it with a real folder to link skills individually. Skipped." >&2
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
      echo "   ! $name: '$dest' already exists and is not a link to '$repo', skipped" >&2
    else
      ln -s "$repo/$name" "$dest"
      echo "   + $name"
    fi
  done
done
