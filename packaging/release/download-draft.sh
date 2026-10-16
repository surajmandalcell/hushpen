#!/bin/bash
# usage: GH_TOKEN=... GITHUB_REPOSITORY=<owner/name> download-draft.sh <tag> <out dir> [--published-ok]
# Downloads every asset of the release for <tag> into <out dir> and prints one line for each.
# Draft releases are not found by tag, so this lists the releases and reads the assets by id.
# Without --published-ok the release must still be a draft.
set -euo pipefail
tag=${1:?usage: download-draft.sh <tag> <out dir> [--published-ok]}
out=${2:?usage: download-draft.sh <tag> <out dir> [--published-ok]}
published_ok=${3:-}
repo=${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is not set}

export TAG=$tag
found=$(gh api "repos/$repo/releases?per_page=100" --paginate \
  --jq '.[] | select(.tag_name == env.TAG) | "\(.id) \(.draft)"' | head -n 1)
if [ -z "$found" ]; then
  echo "no release for $tag in $repo" >&2
  exit 1
fi
read -r id draft <<<"$found"
if [ "$draft" != true ] && [ "$published_ok" != "--published-ok" ]; then
  echo "release $tag is published, not a draft" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out"
count=0
while IFS=$'\t' read -r asset_id name size; do
  gh api -H "Accept: application/octet-stream" "repos/$repo/releases/assets/$asset_id" >"$out/$name"
  got=$(wc -c <"$out/$name" | tr -d ' ')
  if [ "$got" != "$size" ]; then
    echo "download of $name is $got bytes, the release lists $size" >&2
    exit 1
  fi
  echo "downloaded $name ($size bytes)"
  count=$((count + 1))
done < <(gh api "repos/$repo/releases/$id" --jq '.assets[] | [.id, .name, .size] | @tsv')
echo "downloaded $count assets of $tag (draft: $draft)"
