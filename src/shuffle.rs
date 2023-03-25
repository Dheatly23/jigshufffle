use ndarray::parallel::prelude::*;
use ndarray::prelude::*;
use ndarray::Zip;
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

pub(crate) fn shuffle_commands<R: Rng, F>(
    mut mask: ArrayViewMut2<u64>,
    chunk_po2: usize,
    mut random: R,
    mut f: F,
) where
    F: FnMut(Vec<(usize, usize)>, Vec<usize>, usize),
{
    mask_expand(mask.view_mut(), chunk_po2);

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
        indices.shuffle(&mut random);

        f(blocks, indices, m_);
    }
}
