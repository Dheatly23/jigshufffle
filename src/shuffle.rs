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

/// Main jigshuffle algorithm.
///
/// Shuffles an input array with mask and produces shuffled output.
///
/// Parameters:
/// * `arr` : Input array view. Can be multidimensional,
///   but only the first 2 dimension will be shuffled.
/// * `mask` : Mask array view. Must be the same size, otherwise it panics.
/// * `chunk_po2` : Chunk size in exponent of 2. Can be obtained by [usize.ilog2()].
/// * `random` : Random number generator.
pub fn jigshuffle<'a, A, D, R>(
    arr: ArrayView<'_, A, D>,
    mask: ArrayView2<'_, bool>,
    chunk_po2: usize,
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

    mask_expand(mask.view_mut(), chunk_po2);

    let mut out = arr.to_owned();

    for s in (0..=chunk_po2).rev() {
        let m = 1u64 << s;
        let m_ = m as usize;

        let mut blocks: Vec<_> = Zip::indexed(mask.slice(s![..;m_, ..;m_]))
            .into_par_iter()
            .filter_map(|((r, c), v)| {
                if *v & m == m {
                    Some((r * m_, c * m_))
                } else {
                    None
                }
            })
            .collect();
        blocks.sort_unstable();

        #[cfg(debug_assertions)]
        for s in blocks.windows(2) {
            debug_assert_ne!(s[0], s[1]);
        }

        let mut indices: Vec<_> = (0..blocks.len()).collect();
        indices.shuffle(&mut *random);

        let arr = arr.view();
        let out = out.view_mut();
        let blocks = &blocks[..];

        blocks
            .par_iter()
            .enumerate()
            .for_each(move |(i, &(mut r, mut c))| {
                let mut arr = arr.view();
                arr.slice_axis_inplace(Axis(0), Slice::from(r..r + m_));
                arr.slice_axis_inplace(Axis(1), Slice::from(c..c + m_));

                let mut out = out.raw_view();
                (r, c) = blocks[indices[i]];
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
