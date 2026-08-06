#!/usr/bin/env bash

set -euo pipefail

repository="anjing-le/anjing-voicepen"
environment="release"
workflow="release-draft.yml"

if [[ $# -ne 1 || $1 != "--confirm" ]]; then
  echo "Usage: scripts/start-macos-release.sh --confirm" >&2
  echo "This creates a protected Draft Release bound to HEAD. It never publishes the Release." >&2
  exit 2
fi

for command_name in gh git node; do
  command -v "$command_name" >/dev/null || { echo "Required command is unavailable: $command_name" >&2; exit 1; }
done

root=$(git rev-parse --show-toplevel)
cd "$root"
[[ -z "$(git status --porcelain)" ]] || { echo "Worktree must be clean." >&2; exit 1; }
[[ "$(git branch --show-current)" == "main" ]] || { echo "Release must start from main." >&2; exit 1; }
git fetch origin main --quiet
[[ "$(git rev-parse HEAD)" == "$(git rev-parse origin/main)" ]] || { echo "Local main must equal origin/main." >&2; exit 1; }

version=$(node -p "require('./package.json').version")
npm run verify:versions >/dev/null
notes_path="docs/releases/$version.md"
[[ -s "$notes_path" ]] || { echo "Release notes are missing: $notes_path" >&2; exit 1; }
if git show-ref --verify --quiet "refs/tags/v$version"; then
  echo "Local tag v$version already exists." >&2
  exit 1
fi
if tag_response=$(gh api --include "repos/$repository/git/ref/tags/v$version" 2>&1); then
  echo "Remote tag v$version already exists." >&2
  exit 1
else
  tag_status=$(awk 'NR == 1 { print $2 }' <<<"$tag_response")
  [[ "$tag_status" == "404" ]] || { echo "Cannot verify remote tag state (HTTP ${tag_status:-unknown})." >&2; exit 1; }
fi

required_secrets=(
  TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
)
required_variables=(VOICEPEN_UPDATER_PUBKEY)
available_secrets=$(gh secret list --env "$environment" --repo "$repository" --json name --jq '.[].name')
available_variables=$(gh variable list --env "$environment" --repo "$repository" --json name --jq '.[].name')
for name in "${required_secrets[@]}"; do
  grep -qx "$name" <<<"$available_secrets" || { echo "Missing release secret: $name" >&2; exit 1; }
done
for name in "${required_variables[@]}"; do
  grep -qx "$name" <<<"$available_variables" || { echo "Missing release variable: $name" >&2; exit 1; }
done

head_sha=$(git rev-parse HEAD)
ci_conclusion=$(gh run list --repo "$repository" --workflow ci.yml --commit "$head_sha" --limit 1 --json conclusion --jq '.[0].conclusion // ""')
[[ "$ci_conclusion" == "success" ]] || { echo "The latest CI for HEAD is not successful." >&2; exit 1; }

release_notes=$(<"$notes_path")
gh workflow run "$workflow" --repo "$repository" --ref main \
  -f version="$version" \
  -f release_scope=macos \
  -f expected_sha="$head_sha" \
  -f release_notes="$release_notes"

echo "macOS Draft workflow requested for v$version at $head_sha."
echo "Approve the protected release Environment in GitHub, then wait for both macOS jobs and final verification."
