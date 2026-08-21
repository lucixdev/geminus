#!/bin/bash
# This file is part of GEMINUS.
#
# Copyright (C) 2026 lucix.dev <lucix.dev@proton.me>
#
# GEMINUS is free software: you can redistribute it and/or modify it under
# the terms of the GNU General Public License as published by the Free
# Software Foundation, either version 3 of the License, or (at your option)
# any later version.
#
# GEMINUS is distributed in the hope that it will be useful, but WITHOUT ANY
# WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
# details.
#
# You should have received a copy of the GNU General Public License along
# with GEMINUS. If not, see <https://www.gnu.org/licenses/>.
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Post-build patch of the geminus AppImage (tauri-bundler 2.10.x). Four fixes in
# a single extract/repack cycle:
#
#   1) Bundle libgpg-error.so.0. linuxdeploy leaves it out as a "system library"
#      but bundles a recent libgcrypt that needs gpgrt_* @ GPG_ERROR_1.0. On a
#      host whose libgpg-error is older than the build host's, the symbol is
#      missing and the app dies at startup with
#      "libgcrypt.so.20: undefined symbol: gpgrt_add_post_log_func".
#
#   2) Make the .DirIcon symlink relative. tauri-bundler writes it as an absolute
#      path into the build machine, so the icon is invisible on every other host.
#
#   3) Remove the bundled libwayland-*. They live in the AppDir's usr/lib, which
#      comes first in LD_LIBRARY_PATH, so the host's graphics driver loads them
#      instead of its own. A mesa newer than the build host's wayland wants
#      symbols the bundled copy lacks (Fedora 44 / mesa 26: wl_fixes_interface,
#      wl_display_dispatch_queue_timeout): the EGL driver fails to load, the
#      WebKit web process aborts, the window stays blank. These belong to the
#      display server and must come from the host, like libEGL and libGL, which
#      linuxdeploy indeed does not bundle.
#
#   4) Collect the licences of the bundled libraries. They ship from Debian
#      unmodified, so what goes out is the licence file Debian distributes with
#      each one. One dpkg -S call for all of them: one per library would be
#      hundreds of invocations.
#
# Build host: Debian. Fixes 1 and 4 read Debian paths and ask dpkg, so this
# script only runs there.

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
BUNDLE_DIR="$REPO_ROOT/src-tauri/target/release/bundle/appimage"

# Companion of libgcrypt to bundle (see fix 1), at its Debian path.
GPG_ERROR_SRC=/usr/lib/x86_64-linux-gnu/libgpg-error.so.0

# The tool that repacks the AppImage, pinned and checked on every run: the
# "continuous" channel changes under our feet, and this binary builds the
# package we hand to other people.
APPIMAGETOOL_VERSION=1.9.1
APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
APPIMAGETOOL_URL="https://github.com/AppImage/appimagetool/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-x86_64.AppImage"

if ! command -v dpkg >/dev/null 2>&1; then
  echo "ERROR: this script needs a Debian build host — dpkg not found." >&2
  echo "       Fixes 1 and 4 read Debian paths and ask dpkg for licence files." >&2
  exit 1
fi

# Argument: path of the AppImage. Without one, the newest in the bundle
# directory. Never by file name: the name carries the version number, and
# matching on it meant that raising the version skipped every fix above while
# the build still reported success.
APPIMAGE="${1:-}"
if [ -n "$APPIMAGE" ]; then
  if [ ! -f "$APPIMAGE" ]; then
    echo "ERROR: AppImage not found: $APPIMAGE" >&2
    exit 1
  fi
else
  APPIMAGE=$(find "$BUNDLE_DIR" -maxdepth 1 -name '*.AppImage' -printf '%T@ %p\n' 2>/dev/null \
    | sort -rn | head -1 | cut -d' ' -f2- || true)
  if [ -z "$APPIMAGE" ]; then
    echo "ERROR: no AppImage found in $BUNDLE_DIR" >&2
    echo "       Run the build first, or pass the path as an argument." >&2
    exit 1
  fi
fi

APPIMAGE=$(realpath "$APPIMAGE")
APPIMAGE_DIR=$(dirname "$APPIMAGE")
APPIMAGE_NAME=$(basename "$APPIMAGE")
echo "    AppImage: $APPIMAGE_NAME"

# --- Get appimagetool ---
TOOLDIR="$HOME/.cache/geminus-build"
APPIMAGETOOL="$TOOLDIR/appimagetool-${APPIMAGETOOL_VERSION}-x86_64.AppImage"
if [ ! -x "$APPIMAGETOOL" ]; then
  mkdir -p "$TOOLDIR"
  echo ">>> Downloading appimagetool $APPIMAGETOOL_VERSION (once) to $APPIMAGETOOL..."
  curl -fsSL -o "$APPIMAGETOOL.part" "$APPIMAGETOOL_URL"
  mv "$APPIMAGETOOL.part" "$APPIMAGETOOL"
  chmod +x "$APPIMAGETOOL"
fi
if ! echo "$APPIMAGETOOL_SHA256  $APPIMAGETOOL" | sha256sum -c --status; then
  echo "ERROR: appimagetool checksum does not match: $APPIMAGETOOL" >&2
  echo "       Delete it and run again. If it fails twice, do not ship the build." >&2
  exit 1
fi

# --- Extract AppImage into a temp directory ---
WORKDIR=$(mktemp -d -t geminus-appimage-fix-XXXXXX)
trap 'rm -rf "$WORKDIR"' EXIT

echo ">>> Extracting AppImage into $WORKDIR..."
cd "$WORKDIR"
"$APPIMAGE" --appimage-extract > /dev/null

CHANGED=0

# --- Fix 1: bundle libgpg-error.so.0 ---
if [ -e squashfs-root/usr/lib/libgpg-error.so.0 ]; then
  echo "    libgpg-error.so.0 already in the bundle."
else
  if [ ! -e "$GPG_ERROR_SRC" ]; then
    echo "ERROR: $GPG_ERROR_SRC not found on the build host." >&2
    echo "       Expected on Debian; install libgpg-error0." >&2
    exit 1
  fi
  echo ">>> Bundling libgpg-error.so.0 (from $(readlink -f "$GPG_ERROR_SRC" | xargs basename))..."
  cp -L "$GPG_ERROR_SRC" squashfs-root/usr/lib/libgpg-error.so.0
  CHANGED=1
fi

# --- Fix 2: relative .DirIcon symlink ---
if [ ! -L squashfs-root/.DirIcon ]; then
  echo "WARNING: .DirIcon is not a symlink — unexpected layout, skipping the icon fix."
else
  CURRENT_TARGET=$(readlink squashfs-root/.DirIcon)
  echo "    .DirIcon now: $CURRENT_TARGET"
  if [[ "$CURRENT_TARGET" == /* ]]; then
    echo ">>> Patching the .DirIcon symlink (it was absolute)..."
    (cd squashfs-root && rm -f .DirIcon && ln -s usr/share/icons/hicolor/256x256/apps/geminus.png .DirIcon)
    echo "    .DirIcon new: $(readlink squashfs-root/.DirIcon)"
    CHANGED=1
  else
    echo "    -> already relative, no icon fix needed."
  fi
fi

# --- Fix 3: drop the bundled libwayland ---
WAYLAND_BUNDLED=$(find squashfs-root/usr/lib -maxdepth 2 -name 'libwayland-*.so.*' 2>/dev/null || true)
if [ -z "$WAYLAND_BUNDLED" ]; then
  echo "    no bundled libwayland."
else
  echo ">>> Removing the bundled libwayland:"
  while IFS= read -r lib; do
    echo "      $(basename "$lib")"
    rm -f "$lib"
  done <<< "$WAYLAND_BUNDLED"
  CHANGED=1
fi

# --- Fix 4: licences of the bundled libraries ---
LICDIR=squashfs-root/usr/share/doc/third-party-licenses
if [ -d "$LICDIR" ]; then
  echo "    third-party licences already present."
else
  echo ">>> Collecting the licences of the bundled libraries..."
  mkdir -p "$LICDIR"
  mapfile -t SO_PATHS < <(find squashfs-root/usr/lib -maxdepth 1 -name '*.so*' -printf '/usr/lib/x86_64-linux-gnu/%f\n')
  mapfile -t OWNERS < <(dpkg -S "${SO_PATHS[@]}" 2>/dev/null | cut -d: -f1 | tr ',' '\n' | tr -d ' ' | sort -u)
  COPIED=0
  for pkg in "${OWNERS[@]}"; do
    [ -f "/usr/share/doc/$pkg/copyright" ] || continue
    cp "/usr/share/doc/$pkg/copyright" "$LICDIR/$pkg.copyright"
    COPIED=$((COPIED+1))
  done
  echo "    licences copied: $COPIED out of ${#OWNERS[@]} packages"
  if [ "$COPIED" -eq 0 ]; then
    echo "ERROR: no licence collected — nothing ships without them." >&2
    exit 1
  fi
  CHANGED=1
fi

# --- Nothing to do? ---
if [ "$CHANGED" -eq 0 ]; then
  echo ""
  echo "No change needed — the AppImage is already patched."
  exit 0
fi

# --- Repack the AppImage ---
echo ">>> Repacking the AppImage with appimagetool..."
ARCH=x86_64 "$APPIMAGETOOL" --no-appstream squashfs-root "$APPIMAGE_DIR/${APPIMAGE_NAME}.new"

# --- Replace the original ---
mv "$APPIMAGE_DIR/${APPIMAGE_NAME}.new" "$APPIMAGE"
chmod +x "$APPIMAGE"

echo ""
echo "OK — AppImage patched: $APPIMAGE"
