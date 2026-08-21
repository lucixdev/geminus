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

# Full release build: cargo tauri build, then the AppImage post-build patch
# (bundled companion library, icon, display-server libraries, third-party
# licences — see fix-appimage-icon.sh).
# Usage: ./scripts/build-release.sh (from the repo root or anywhere else)
#
# Build host: Debian, because of the patch script.

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)

echo ">>> cargo tauri build"
cd "$REPO_ROOT/src-tauri"
cargo tauri build

# The patch script finds the AppImage by itself and stops the build if it cannot
# apply the fixes: a build that skipped them is not a build that succeeded — it
# would ship a broken icon and no third-party licences.
echo ""
echo ">>> AppImage post-build patch"
"$SCRIPT_DIR/fix-appimage-icon.sh"

echo ""
echo "Build complete. Bundles in: $REPO_ROOT/src-tauri/target/release/bundle/"
