#!/usr/bin/env bash
# Rename this template to a real app: crate names, Rust paths, bundle
# identifier, systemd/deb paths, Android package and docs, in one pass.
#
#   ./scripts/rename-app.sh --name ScApp --slug sc-app \
#       --identifier com.nicmell.sc.app [--repo sc-app3] [--dry-run]
#
# Only git-tracked text files are touched (binaries are skipped), so
# `git diff` is the full record of the rename.
set -euo pipefail

# The template's own names — the left-hand side of every rename below.
OLD_SLUG="tauri-leptos"          # crate prefix, leptos output name, deb/etc dirs
OLD_SNAKE="tauri_leptos"         # Rust module paths, Android theme
OLD_IDENT="com.nick.tauri-leptos"
OLD_ANDROID_PKG="com.nick.tauri_leptos"
OLD_APP_DIR_ENV="TAURI_LEPTOS_APP_DIR"

usage() {
  sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

NAME=""      # display name (window title, productName, Android app_name)
SLUG=""      # kebab-case: crate prefix, binary, /etc/<slug>, site pkg name
IDENT=""     # bundle identifier
REPO=""      # checkout directory name in the docs (defaults to the slug)
DRY_RUN=0
NO_CARGO=0

while [ $# -gt 0 ]; do
  case "$1" in
    --name) NAME="${2:?}"; shift 2 ;;
    --slug) SLUG="${2:?}"; shift 2 ;;
    --identifier) IDENT="${2:?}"; shift 2 ;;
    --repo) REPO="${2:?}"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    --no-cargo) NO_CARGO=1; shift ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage 1 ;;
  esac
done

[ -n "$NAME" ] && [ -n "$SLUG" ] && [ -n "$IDENT" ] || usage 1
case "$SLUG" in
  [a-z]*) : ;;
  *) echo "--slug must start with a lowercase letter: $SLUG" >&2; exit 1 ;;
esac
case "$SLUG" in
  *[^a-z0-9-]*) echo "--slug must be kebab-case [a-z0-9-]: $SLUG" >&2; exit 1 ;;
esac
case "$IDENT" in
  *.*) : ;;
  *) echo "--identifier must be reverse-DNS: $IDENT" >&2; exit 1 ;;
esac

cd "$(git rev-parse --show-toplevel)"

SNAKE="$(printf '%s' "$SLUG" | tr '-' '_')"
APP_DIR_ENV="$(printf '%s' "$SNAKE" | tr '[:lower:]' '[:upper:]')_DIR"
# Tauri sanitises the identifier the same way for the Android package.
ANDROID_PKG="$(printf '%s' "$IDENT" | tr '-' '_')"
ANDROID_PATH="$(printf '%s' "$ANDROID_PKG" | tr '.' '/')"
OLD_ANDROID_PATH="$(printf '%s' "$OLD_ANDROID_PKG" | tr '.' '/')"
REPO="${REPO:-$SLUG}"

run() {
  if [ "$DRY_RUN" = 1 ]; then
    printf 'would run:'; printf ' %q' "$@"; printf '\n'
  else
    "$@"
  fi
}

# Literal (non-regex) replacement across the given files.
replace() {
  local from="$1" to="$2"; shift 2
  [ $# -gt 0 ] || return 0
  run perl -pi -e 'BEGIN { ($f, $t) = splice(@ARGV, 0, 2) } s/\Q$f\E/$t/g' \
    "$from" "$to" "$@"
}

# One spot in one file: the name as shown to a human, not as an identifier.
replace_in() {
  local file="$1" from="$2" to="$3"
  [ -f "$file" ] || return 0
  replace "$from" "$to" "$file"
}

# Every tracked text file except this script, which holds the old names
# on purpose. grep -I drops the icons and the gradle jar.
files=()
while IFS= read -r f; do
  files+=("$f")
done < <(git ls-files -z -- . ':!:scripts/rename-app.sh' \
  | xargs -0 grep -Il . 2>/dev/null || true)
[ "${#files[@]}" -gt 0 ] || { echo "no tracked text files found" >&2; exit 1; }

echo "renaming ${OLD_SLUG} -> ${SLUG} (${NAME}, ${IDENT}), ${#files[@]} files"

android_strings="src-tauri/gen/android/app/src/main/res/values/strings.xml"
replace_in src-tauri/tauri.conf.json "\"productName\": \"$OLD_SLUG\"" "\"productName\": \"$NAME\""
replace_in src-tauri/src/lib.rs ".title(\"$OLD_SLUG\")" ".title(\"$NAME\")"
replace_in "$android_strings" "\"$OLD_SLUG\"" "\"$NAME\""
replace_in README.md "# $OLD_SLUG" "# $NAME"
replace_in CLAUDE.md "# $OLD_SLUG" "# $NAME"

# Longest match first: the -cli/-core/-ui crates before the bare slug.
replace "${OLD_SNAKE}_lib"   "${SNAKE}_lib"        "${files[@]}"
replace "${OLD_SNAKE}_core"  "${SNAKE}_core"       "${files[@]}"
replace "${OLD_SNAKE}_ui"    "${SNAKE}_ui"         "${files[@]}"
replace "${OLD_SLUG}-cli"    "${SLUG}-cli"         "${files[@]}"
replace "${OLD_SLUG}-core"   "${SLUG}-core"        "${files[@]}"
replace "${OLD_SLUG}-ui"     "${SLUG}-ui"          "${files[@]}"
replace "$OLD_APP_DIR_ENV"   "$APP_DIR_ENV"        "${files[@]}"
replace "$OLD_IDENT"         "$IDENT"              "${files[@]}"
replace "$OLD_ANDROID_PKG"   "$ANDROID_PKG"        "${files[@]}"
replace "Theme.${OLD_SNAKE}" "Theme.${SNAKE}"      "${files[@]}"
replace "cd ${OLD_SLUG}"     "cd ${REPO}"          "${files[@]}"
replace "$OLD_SNAKE"         "$SNAKE"              "${files[@]}"
replace "$OLD_SLUG"          "$SLUG"               "${files[@]}"

# Paths that carry the old name: the systemd unit and the Android package
# directories (one under app/, one under buildSrc/).
move() {
  local from="$1" to="$2"
  [ -e "$from" ] || return 0
  if [ "$from" = "$to" ]; then return 0; fi
  run mkdir -p "$(dirname "$to")"
  run git mv "$from" "$to"
}

move "crates/app-cli/debian/${OLD_SLUG}-cli.service" \
     "crates/app-cli/debian/${SLUG}-cli.service"
for java_root in src-tauri/gen/android/app/src/main/java \
                 src-tauri/gen/android/buildSrc/src/main/java; do
  move "${java_root}/${OLD_ANDROID_PATH}" "${java_root}/${ANDROID_PATH}"
done

if [ "$DRY_RUN" = 1 ]; then exit 0; fi

# Moving the package leaf out leaves its old spine behind.
find src-tauri/gen/android -type d -empty -delete 2>/dev/null || true

# Cargo.lock was edited in place above; let cargo re-sort the renamed
# packages so the next build does not produce a stray diff.
if [ "$NO_CARGO" = 0 ] && command -v cargo >/dev/null; then
  cargo metadata --format-version 1 --offline >/dev/null 2>&1 \
    || echo "note: could not refresh Cargo.lock offline; a build will re-sort it" >&2
fi

leftovers=$(git ls-files -- . ':!:scripts/rename-app.sh' \
  | xargs grep -IlF -e "$OLD_SLUG" -e "$OLD_SNAKE" -e "$OLD_APP_DIR_ENV" \
      -e "${OLD_IDENT%%.*}.nick" 2>/dev/null || true)
if [ -n "$leftovers" ]; then
  echo "leftover template names in:" >&2
  printf '  %s\n' $leftovers >&2
  exit 1
fi

echo "done. review with: git status && git diff"
