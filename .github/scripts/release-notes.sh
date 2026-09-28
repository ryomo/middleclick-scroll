#!/usr/bin/env bash
# Build release notes from Conventional Commits between two refs.
#
# Usage: release-notes.sh <new-tag> [<from-ref>] [<to-ref>]
#   <from-ref> defaults to the previous tag (all history if none).
#   <to-ref>   defaults to HEAD.
#
# feat -> Features, fix -> Bug Fixes, docs and chore(release) -> omitted,
# anything else -> Other Changes.
set -euo pipefail

new_tag=${1:?usage: release-notes.sh <new-tag> [<from-ref>] [<to-ref>]}
to_ref=${3:-HEAD}
from_ref=${2:-$(git describe --tags --abbrev=0 "$to_ref" 2>/dev/null || true)}

range=$to_ref
if [[ -n $from_ref ]]; then
  range="$from_ref..$to_ref"
fi

features=()
fixes=()
others=()
re='^([a-zA-Z]+)(\([^)]*\))?(!)?: (.*)$'

while IFS= read -r subject; do
  if [[ $subject =~ $re ]]; then
    type=${BASH_REMATCH[1],,}
    scope=${BASH_REMATCH[2]}
    desc=${BASH_REMATCH[4]}
    [[ $type == chore && $scope == "(release)" ]] && continue
    case $type in
      feat) features+=("$desc") ;;
      fix) fixes+=("$desc") ;;
      docs) ;;
      *) others+=("$subject") ;;
    esac
  else
    others+=("$subject")
  fi
done < <(git log --no-merges --reverse --format=%s "$range")

print_section() {
  local title=$1
  shift
  (($# == 0)) && return
  echo "## $title"
  printf -- '- %s\n' "$@"
  echo
}

print_section "Features" "${features[@]}"
print_section "Bug Fixes" "${fixes[@]}"
print_section "Other Changes" "${others[@]}"

if [[ -n $from_ref && -n ${GITHUB_REPOSITORY:-} ]]; then
  echo "**Full Changelog**: https://github.com/$GITHUB_REPOSITORY/compare/$from_ref...$new_tag"
fi
