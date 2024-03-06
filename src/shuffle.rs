// Copyright (C) 2023 Dheatly23
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use ndarray::parallel::prelude::*;
use ndarray::prelude::*;
use ndarray::{Slice, Zip};
use rand::prelude::*;

/// Configuration for [`jigshuffle`].
#[derive(Debug, Clone)]
pub struct Config {
    chunk_po2: usize,
    flip: bool,
    rotate: bool,
}

/// Config builder.
pub struct ConfigBuilder(Config);

#[allow(dead_code)]
impl ConfigBuilder {
    /// Create new [`ConfigBuilder`]
    pub fn new() -> Self {
        Self(Config {
            chunk_po2: 0,
            flip: false,
            rotate: false,
        })
    }

    /// Sets chunk size as power-of-2.
    ///
    /// **Panics if v >= 64**
    pub fn chunk_po2(mut self, v: usize) -> Self {
        if v >= 64 {
            panic!("Chunk size cannot be >= 2^64 (got 2^{v})");
        }
        self.0.chunk_po2 = v;
        self
    }

    /// Sets chunk size.
    ///
    /// Value will be rounded down to power-of-2.
    pub fn chunk(mut self, v: u64) -> Self {
        self.0.chunk_po2 = v.ilog2() as _;
        self
    }

    /// Sets flag werether it will rotate blocks.
    pub fn rotate(mut self, v: bool) -> Self {
        self.0.rotate = v;
        self
    }

    /// Sets flag werether it will flip blocks.
    pub fn flip(mut self, v: bool) -> Self {
        self.0.flip = v;
        self
    }

    /// Build config.
    pub fn build(self) -> Config {
        self.0
    }
}

fn mask_expand(mut mask: ArrayViewMut2<u64>, chunk_po2: usize) {
    for s in 0..chunk_po2 {
        let m = 1u64 << s;
        let m_ = m << 1;
        let i = m as usize;

        par_azip!((mut a in mask.exact_chunks_mut((m_ as usize, m_ as usize))) {
            if m & a[[0, 0]] & a[[0, i]] & a[[i, 0]] & a[[i, i]] == m {
                a[[0, 0]] = m_;
                a[[0, i]] = 0;
                a[[i, 0]] = 0;
                a[[i, i]] = 0;
            }
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Noop,
    FlipX,
    FlipY,
    FlipBoth,
    Swap,
    SwapFlipX,
    SwapFlipY,
    SwapFlipBoth,
}

/// Main jigshuffle algorithm.
///
/// Shuffles an input array with mask and produces shuffled output.
///
/// Parameters:
/// * `arr` : Input array view. Can be multidimensional,
///   but only the first 2 dimension will be shuffled.
/// * `mask` : Mask array view. Must be the same size, otherwise it panics.
/// * `config` : Configuration options.
/// * `random` : Random number generator.
pub fn jigshuffle<'a, A, D, R>(
    arr: ArrayView<'_, A, D>,
    mask: ArrayView2<'_, bool>,
    config: &Config,
    random: &mut R,
) -> Array<A, D>
where
    A: 'a + Clone + Send + Sync,
    D: Dimension,
    R: Rng,
{
    if &arr.shape()[..2] != mask.shape() {
        let s1 = arr.shape();
        let s2 = mask.shape();
        panic!(
            "Array shape mismatch with mask ([{} {}] != [{} {}])",
            s1[0], s1[1], s2[0], s2[1],
        );
    }

    let mut mask: Array2<u64> = mask.mapv(|v| if v { 0 } else { 1 });

    mask_expand(mask.view_mut(), config.chunk_po2);

    let mut out = arr.to_owned();

    for s in (0..=config.chunk_po2).rev() {
        let m = 1u64 << s;
        let m_ = m as usize;

        let mut blocks: Vec<_> = Zip::indexed(mask.slice(s![..;m_, ..;m_]))
            .into_par_iter()
            .filter_map(|((r, c), v)| {
                if *v & m == m {
                    Some((r * m_, c * m_, Mode::Noop))
                } else {
                    None
                }
            })
            .collect();
        blocks.sort_unstable_by_key(|&(r, c, _)| (r, c));
        for (_, _, mode) in &mut blocks {
            *mode = match (config.flip, config.rotate) {
                (false, false) => break,
                (true, false) => match random.gen_range(0u32..4) {
                    0 => Mode::Noop,
                    1 => Mode::SwapFlipX,
                    2 => Mode::SwapFlipY,
                    3 => Mode::FlipBoth,
                    v => unreachable!("Out-of-bound value {v}"),
                },
                (false, true) => match random.gen_range(0u32..4) {
                    0 => Mode::Noop,
                    1 => Mode::FlipX,
                    2 => Mode::FlipY,
                    3 => Mode::FlipBoth,
                    v => unreachable!("Out-of-bound value {v}"),
                },
                (true, true) => match random.gen_range(0u32..8) {
                    0 => Mode::Noop,
                    1 => Mode::FlipX,
                    2 => Mode::FlipY,
                    3 => Mode::FlipBoth,
                    4 => Mode::Swap,
                    5 => Mode::SwapFlipX,
                    6 => Mode::SwapFlipY,
                    7 => Mode::SwapFlipBoth,
                    v => unreachable!("Out-of-bound value {v}"),
                },
            };
        }

        #[cfg(debug_assertions)]
        for s in blocks.windows(2) {
            let (r0, c0, _) = &s[0];
            let (r1, c1, _) = &s[1];
            debug_assert_ne!((r0, c0), (r1, c1));
        }

        let mut indices: Vec<_> = (0..blocks.len()).collect();
        indices.shuffle(&mut *random);

        let arr = arr.view();
        let out = out.view_mut();
        let blocks = &blocks[..];

        blocks
            .par_iter()
            .enumerate()
            .for_each(move |(i, &(mut r, mut c, mode))| {
                let mut arr = arr.view();
                arr.slice_axis_inplace(Axis(0), Slice::from(r..r + m_));
                arr.slice_axis_inplace(Axis(1), Slice::from(c..c + m_));

                match mode {
                    Mode::Noop => (),
                    Mode::FlipX => arr.invert_axis(Axis(1)),
                    Mode::FlipY => arr.invert_axis(Axis(0)),
                    Mode::FlipBoth => {
                        arr.invert_axis(Axis(0));
                        arr.invert_axis(Axis(1));
                    }
                    Mode::Swap => arr.swap_axes(0, 1),
                    Mode::SwapFlipX => {
                        arr.swap_axes(0, 1);
                        arr.invert_axis(Axis(1));
                    }
                    Mode::SwapFlipY => {
                        arr.swap_axes(0, 1);
                        arr.invert_axis(Axis(0));
                    }
                    Mode::SwapFlipBoth => {
                        arr.swap_axes(0, 1);
                        arr.invert_axis(Axis(0));
                        arr.invert_axis(Axis(1));
                    }
                }

                let mut out = out.raw_view();
                (r, c, _) = blocks[indices[i]];
                out.slice_axis_inplace(Axis(0), Slice::from(r..r + m_));
                out.slice_axis_inplace(Axis(1), Slice::from(c..c + m_));

                // SAFETY: Output slices is guaranteed to be non-overlapping
                azip!((d in out, s in arr) unsafe {
                    (*(d as *mut A)).clone_from(s)
                });
            });
    }

    out
}
