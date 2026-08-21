/* This file is part of GEMINUS.
 *
 * Copyright (C) 2026 lucix.dev <lucix.dev@proton.me>
 *
 * GEMINUS is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * GEMINUS is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
 * details.
 *
 * You should have received a copy of the GNU General Public License along
 * with GEMINUS. If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

// System boundary: everything that only works on one OS lives behind here,
// one implementation per system with the same shape. The rest of the project
// calls these and does not know which system it runs on.

#[cfg(unix)]
mod linux;
#[cfg(unix)]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
