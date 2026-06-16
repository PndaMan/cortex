#!/usr/bin/env bash
# migrate-cortex-data.sh — one-time data migration for the imperative -> declarative cutover.
#
# The OLD homelab stack runs as ROOTLESS podman under user `aidan`; the NEW
# declarative stack (cortex.nix) runs as ROOTFUL podman (system units). Their
# named volumes share the SAME names but live in DIFFERENT storage, so podman
# will NOT see the old data automatically. This copies the old volume contents
# into the new (system) volumes so nothing is lost — your WebDAV sync library
# (cortex-sync-data) above all, plus the cached Whisper/Ollama models.
#
# Run as root on aether AFTER `nixos-rebuild switch` has created the new units,
# with BOTH stacks stopped:
#
#   # stop the NEW system units:
#   sudo systemctl stop 'podman-cortex-*'
#   # stop the OLD rootless user units:
#   sudo -u aidan XDG_RUNTIME_DIR=/run/user/$(id -u aidan) \
#        systemctl --user stop 'container-cortex-*'
#   # copy the data:
#   sudo ./migrate-cortex-data.sh
#   # bring the new stack back up:
#   sudo systemctl start podman-cortex-proxy   # others are pulled in by deps
#
# Idempotent-ish: it appends/overwrites files in the destination; re-running is
# safe but will re-copy. It NEVER deletes the source, so the old data stays put
# until you're satisfied.

set -euo pipefail

OLD_USER=aidan
VOLUMES=(cortex-sync-data cortex-whisper-models cortex-ollama-models)

[ "$(id -u)" -eq 0 ] || { echo "Run as root (sudo)." >&2; exit 1; }
OLD_UID=$(id -u "$OLD_USER")

# Run podman as the old rootless user to find its volume mountpoints.
rootless() { sudo -u "$OLD_USER" XDG_RUNTIME_DIR="/run/user/$OLD_UID" podman "$@"; }

for v in "${VOLUMES[@]}"; do
  printf '>> %s\n' "$v"
  if ! src=$(rootless volume inspect "$v" --format '{{.Mountpoint}}' 2>/dev/null); then
    echo "   old volume '$v' not found under user $OLD_USER — skipping"
    continue
  fi
  # Make sure the new (system) volume exists, then resolve its mountpoint.
  podman volume inspect "$v" >/dev/null 2>&1 || podman volume create "$v" >/dev/null
  dst=$(podman volume inspect "$v" --format '{{.Mountpoint}}')

  if [ -z "$(ls -A "$src" 2>/dev/null)" ]; then
    echo "   source empty, nothing to copy"
    continue
  fi
  printf '   %s\n   -> %s\n' "$src" "$dst"
  # cp -a preserves perms/ownership/timestamps; copy contents (including dotfiles).
  cp -a "$src"/. "$dst"/
done

echo
echo "Done. Verify the WebDAV sync library is intact (e.g. PROPFIND /sync/),"
echo "then start the system units. The OLD rootless volumes are untouched."
