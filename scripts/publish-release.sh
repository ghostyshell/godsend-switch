#!/bin/bash
# Upload release artifacts to GoFile (primary) and file.kiwi (backup), rewrite
# the README download table, commit and push.
#
#   scripts/publish-release.sh --artifacts /path/to/dist
#   scripts/publish-release.sh                 # build the current version first
#
# Environment:
#   GOFILE_TOKEN   overrides .gofile-io-token
#   SKIP_PUSH=1    commit but do not push
set -euo pipefail

SELF=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$SELF/.." && pwd)
cd "$REPO"

ARTIFACTS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --artifacts) ARTIFACTS=$2; shift 2 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

log() { printf '[%s] %s\n' "$(date '+%F %T')" "$*" >&2; }
die() { log "FAILED: $*"; exit 1; }

for tool in python3 curl git shasum tar; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool not found on PATH ($PATH)"
done

V=$(python3 - <<'PY'
import json
print(json.load(open("app/src-tauri/tauri.conf.json"))["version"])
PY
)
[ -n "$V" ] || die "could not read the version"

RUN=".release-${V}-$(date +%Y%m%d)"
LINKS="$RUN/links.txt"
mkdir -p "$RUN"
touch "$LINKS"

LOCK=.release-publish.lock
if ! mkdir "$LOCK" 2>/dev/null; then
  pid=$(cat "$LOCK/pid" 2>/dev/null || true)
  if [ -n "${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then
    log "another publish is already running (pid $pid) - exiting"
    exit 0
  fi
  rm -rf "$LOCK"
  mkdir "$LOCK"
fi
printf '%s\n' "$$" > "$LOCK/pid"
trap 'rm -rf "$LOCK"' EXIT

find . -maxdepth 1 -name '.release-*' -type d -mtime +30 -exec rm -rf {} + 2>/dev/null || true

TOKEN=${GOFILE_TOKEN:-$(cat "$REPO/.gofile-io-token" 2>/dev/null || true)}
[ -n "$TOKEN" ] || die "no GoFile token: write .gofile-io-token or set GOFILE_TOKEN"

log "publishing GODsend Switch $V"

# --- 1. Artifacts ---------------------------------------------------------------
if [ -n "$ARTIFACTS" ]; then
  [ -d "$ARTIFACTS" ] || die "no artifact directory at $ARTIFACTS"
  DIST=$(cd "$ARTIFACTS" && pwd)
else
  DIST="$RUN/artifacts"
  rm -rf "$DIST"
  mkdir -p "$DIST"
  command -v gh >/dev/null 2>&1 || die "gh not found (needed to run the CI build)"
  SLUG=ghostyshell/godsend-switch
  log "dispatching the all-platform CI build"
  gh workflow run build.yml --repo "$SLUG" || die "could not dispatch the build"
  RUN_ID=""
  for _ in $(seq 1 60); do
    RUN_ID=$(gh run list --repo "$SLUG" --workflow build --limit 3 \
      --json databaseId,status,event \
      --jq '[.[] | select(.event=="workflow_dispatch" and (.status=="queued" or .status=="in_progress" or .status=="requested"))][0].databaseId' 2>/dev/null || true)
    [ -n "$RUN_ID" ] && break
    sleep 5
  done
  [ -n "$RUN_ID" ] || die "the dispatched build never appeared"
  log "waiting for CI run $RUN_ID"
  # Do not use `gh run watch`: it gives up when the API rate-limits a busy
  # shared token, which looks like a failed build. Poll the run state instead
  # and retry through transient API errors.
  deadline=$((SECONDS + 5400))
  while :; do
    state=$(gh run view "$RUN_ID" --repo "$SLUG" --json status,conclusion \
      --jq '"\(.status) \(.conclusion // "-")"' 2>/dev/null || true)
    case "$state" in
      "completed success") break ;;
      "completed "*) die "CI run $RUN_ID finished: $state" ;;
    esac
    [ "$SECONDS" -ge "$deadline" ] && die "CI run $RUN_ID did not finish in 90 minutes"
    log "  CI state: ${state:-api busy}; retrying in 30s"
    sleep 30
  done
  rm -rf "$DIST"
  mkdir -p "$DIST"
  ok=0
  for attempt in 1 2 3; do
    if gh run download "$RUN_ID" --repo "$SLUG" --dir "$DIST"; then ok=1; break; fi
    log "  artifact download attempt $attempt/3 failed; retrying in 30s"
    sleep 30
  done
  [ "$ok" = "1" ] || die "artifact download failed after 3 attempts"
  # Flatten the per-OS subdirectories.
  find "$DIST" -mindepth 2 -type f -exec mv {} "$DIST/" \; 2>/dev/null || true
  find "$DIST" -mindepth 1 -type d -empty -delete 2>/dev/null || true
fi

FILES=()
for f in "$DIST"/*.dmg "$DIST"/*.tar.gz "$DIST"/*.zip "$DIST"/*.AppImage "$DIST"/*.deb "$DIST"/*setup.exe; do
  [ -f "$f" ] && FILES+=("$(basename "$f")")
done
[ ${#FILES[@]} -gt 0 ] || die "no release files in $DIST"
# Normalize CI names to godsend-switch-<version>-<platform>.<ext>.
normalize() {
  local f=$1 base
  base=$(basename "$f")
  case "$base" in
    godsend-switch-macos-*)     mv "$f" "$DIST/godsend-switch-${V}-${base#godsend-switch-macos-}" ;;
    godsend-switch-cli-*)       case "$base" in *"${V}"*) : ;; *) mv "$f" "$DIST/godsend-switch-cli-${V}-${base#godsend-switch-cli-}" ;; esac ;;
    "GODsend Switch_"*amd64.AppImage) mv "$f" "$DIST/godsend-switch-${V}-linux-x64.AppImage" ;;
    "GODsend Switch_"*amd64.deb)      mv "$f" "$DIST/godsend-switch-${V}-linux-x64.deb" ;;
    "GODsend Switch_"*x64-setup.exe)  mv "$f" "$DIST/godsend-switch-${V}-x64-setup.exe" ;;
  esac
}
for f in "${FILES[@]}"; do normalize "$DIST/$f"; done
FILES=()
for f in "$DIST"/*.dmg "$DIST"/*.tar.gz "$DIST"/*.zip "$DIST"/*.AppImage "$DIST"/*.deb "$DIST"/*setup.exe; do
  [ -f "$f" ] && FILES+=("$(basename "$f")")
done
log "artifacts: ${FILES[*]}"

# --- 2. Upload ------------------------------------------------------------------
field() { grep -F "$1 |" "$LINKS" 2>/dev/null | tail -1 | cut -d'|' -f"$2" | sed 's/^ *//;s/ *$//' || true; }
row() {
  grep -v -F "$1 |" "$LINKS" > "$LINKS.tmp" 2>/dev/null || true
  mv "$LINKS.tmp" "$LINKS"
  printf '%s | %s | %s | %s | %s\n' "$1" "$2" "$3" "$4" "$5" >> "$LINKS"
}

human_size() {
  python3 - "$1" <<'PY'
import os, sys
b = os.path.getsize(sys.argv[1])
for unit in ("B", "KiB", "MiB", "GiB"):
    if b < 1024 or unit == "GiB":
        print(f"{b:.1f} {unit}" if unit != "B" else f"{b} B")
        break
    b /= 1024.0
PY
}

put_gofile() {
  local f=$1 url=""
  for attempt in 1 2 3; do
    url=$(curl -sS --max-time 3600 -X POST \
      "https://upload.gofile.io/contents/uploadfile" \
      -H "Authorization: Bearer $TOKEN" \
      -F "file=@$DIST/$f" 2>/dev/null \
      | python3 -c 'import json,sys
try:
    print(json.load(sys.stdin).get("data", {}).get("downloadPage", ""))
except Exception:
    print("")' 2>/dev/null) || url=""
    if [ -n "$url" ]; then printf '%s\n' "$url"; return 0; fi
    log "  gofile attempt $attempt/3 failed for $f; retrying in 30s"
    sleep 30
  done
  return 1
}

put_kiwi() {
  FORCE_COLOR=0 npx -y @file-kiwi/node "$DIST/$1" --title "$1" 2>&1 \
    | grep -Eo 'https://file\.kiwi/[A-Za-z0-9][A-Za-z0-9#_+/=-]*' | tail -1 || true
}

incomplete=0
for f in "${FILES[@]}"; do
  gf=$(field "$f" 2)
  kw=$(field "$f" 3)
  if [ -n "$gf" ] && [ "$gf" != "EMPTY" ] && [ -n "$kw" ] && [ "$kw" != "EMPTY" ]; then
    log "[skip] $f already uploaded"
    continue
  fi
  size=$(human_size "$DIST/$f")
  sha=$(shasum -a 256 "$DIST/$f" | cut -d' ' -f1)
  log "[start] $f ($size)"
  if [ -z "$gf" ] || [ "$gf" = "EMPTY" ]; then
    gf=$(put_gofile "$f") || gf="EMPTY"
  fi
  if [ -z "$kw" ] || [ "$kw" = "EMPTY" ]; then
    kw=$(put_kiwi "$f")
  fi
  [ -n "$gf" ] && [ "$gf" != "EMPTY" ] || incomplete=1
  [ -n "$kw" ] && [ "$kw" != "EMPTY" ] || incomplete=1
  log "[done] $f | ${gf:-EMPTY} | ${kw:-EMPTY}"
  row "$f" "${gf:-EMPTY}" "${kw:-EMPTY}" "$size" "$sha"
done

# --- 3. Rewrite the README table -------------------------------------------------
python3 -I scripts/patch-release-links.py "$LINKS" || die "could not patch README"

# --- 4. Commit and push ----------------------------------------------------------
if git diff --quiet -- README.md docs/downloads.json; then
  log "download links already current - nothing to commit"
else
  git add README.md docs/downloads.json
  git commit -q -m "docs: refresh ${V} download links (GoFile + file.kiwi)

Weekly mirror refresh: the current version is rebuilt and re-uploaded to
GoFile (primary) and file.kiwi (backup). Free-tier links expire, so the
README table is repointed at the fresh mirrors." || die "git commit failed"
  if [ "${SKIP_PUSH:-0}" = "1" ]; then
    log "SKIP_PUSH=1: links committed locally; not pushed"
  else
    git push -q origin main || die "git push failed"
    log "pushed refreshed ${V} links"
    # Prove the live site actually serves the new feed. Pages caches for ten
    # minutes, so a cache-busted request is the only reliable check.
    log "verifying the live site feed"
    sleep 20
    LIVE=$(curl -fsS "https://ghostyshell.github.io/godsend-switch/downloads.json?v=$(date +%s)" 2>/dev/null \
      | python3 -c 'import json,sys
try:
    print(json.load(sys.stdin).get("version",""))
except Exception:
    print("")' 2>/dev/null || true)
    if [ "$LIVE" = "$V" ]; then
      log "live site feed serves $V"
    else
      log "WARNING: live site feed shows '${LIVE:-unknown}' - Pages may need a minute; re-run to confirm"
    fi
  fi
fi

if [ "$incomplete" -ne 0 ]; then
  log "WARNING: at least one mirror is missing - run again to finish"
else
  log "all mirrors uploaded"
fi
log "done"
