//! Library to semi-destructively shuffle image/array.
//!
//! This library defines one function, [jigshuffle].
//! It's used to shuffle any array (with mask), and shuffle it such that:
//!
//! * No element is duplicated nor removed.
//! * All elements in a block is moved together.
//! * All blocks occupies it's maximum size that satisfies mask
//!   and power-of-2 coordinate.
//!
//! This can be used to create interesting shuffle/censor, where
//! all the data is preserved, while being not very obvious.

// Copyright (C) 2023 Dheatly23
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//

mod shuffle;

#[doc(inline)]
pub use crate::shuffle::jigshuffle;
