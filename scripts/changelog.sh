#!/usr/bin/env bash
#
# Release notes from the commit log since the last release tag.
#
# The output is Markdown, one bullet per commit, and is what the updater shows
# as "What's new": it becomes the `notes` field of the update manifest
# (latest.json) that the app fetches, so a user sees exactly what changed since
# the version they are running before installing.
#
# Uses Jujutsu (jj) when the working copy is a jj repo, and falls back to git.
# A tag argument pins the "since" point; without one it uses the most recent
# tag, and if there are no tags it lists the whole history.
#
#   scripts/changelog.sh            # since the latest tag
#   scripts/changelog.sh v0.1.0     # since a specific tag
#
set -euo pipefail
cd "$(dirname "$0")/.."

since="${1:-}"
if [ -z "$since" ]; then
  since="$(git describe --tags --abbrev=0 2>/dev/null || true)"
fi

if command -v jj >/dev/null 2>&1 && jj root >/dev/null 2>&1; then
  # jj revset: commits reachable from the working-copy parent (@-) but not from
  # the tag, newest first, skipping empty and merge commits. `description` is
  # truthy only for commits that actually have a message.
  if [ -n "$since" ]; then
    rev="${since}..@- ~ empty() ~ merges()"
  else
    rev="::@- ~ empty() ~ merges()"
  fi
  jj log --no-graph --no-pager --reversed -r "$rev" \
    --template 'if(description, "- " ++ description.first_line() ++ "\n")'
else
  if [ -n "$since" ]; then
    git log --no-merges --reverse --pretty=format:'- %s' "${since}..HEAD"
  else
    git log --no-merges --reverse --pretty=format:'- %s'
  fi
  echo
fi
