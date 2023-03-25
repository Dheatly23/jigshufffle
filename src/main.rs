mod shuffle;

use std::fs::File;
use std::io::BufReader;
use std::mem;
use std::path::PathBuf;

use anyhow::Error;
use clap::Parser;
use image::io::Reader as ImageReader;
use image::{save_buffer, DynamicImage, Luma};
use ndarray::parallel::prelude::*;
use ndarray::prelude::*;
use rand::SeedableRng;
use sha2::{Digest, Sha256};

#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    /// Input file
    input: PathBuf,

    /// Mask file
    #[arg(short = 'm', long)]
    mask: Option<PathBuf>,

    /// Tile size (must be power of 2)
    #[arg(short = 't', long)]
    tile_size: usize,

    /// Random seed
    #[arg(long)]
    seed: Option<String>,

    /// Output file
    #[arg(short = 'o', long)]
    output: PathBuf,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let tile_size = args.tile_size.ilog2() as _;
    if (1usize << tile_size) != args.tile_size {
        println!(
            "WARNING: Tile size is not power of 2, using {}",
            1usize << tile_size
        );
    }

    let random = if let Some(seed) = args.seed {
        let mut hasher = Sha256::new();
        hasher.update(seed);

        rand_xoshiro::Xoshiro256StarStar::from_seed(hasher.finalize().into())
    } else {
        rand_xoshiro::Xoshiro256StarStar::from_entropy()
    };

    let im = ImageReader::new(BufReader::new(File::open(&args.input)?))
        .with_guessed_format()?
        .decode()?;

    let mut mask = if let Some(p) = args.mask {
        let mi = ImageReader::new(BufReader::new(File::open(p)?))
            .with_guessed_format()?
            .decode()?
            .into_luma8();

        let mut arr = <Array2<u64>>::zeros((im.height() as usize, im.width() as usize));
        par_azip!((index (i, j), a in &mut arr) {
            *a = match mi.get_pixel_checked(i as _, j as _) {
                None | Some(Luma([0])) => 0,
                _ => 1,
            };
        });
        arr
    } else {
        <Array2<u64>>::ones((im.height() as usize, im.width() as usize))
    };

    let mut out = im.as_bytes().to_vec();

    let f = {
        let (x_stride, y_stride): (usize, usize) = match &im {
            DynamicImage::ImageLuma8(im) => (1, im.width() as usize),
            DynamicImage::ImageLumaA8(im) => (2, im.width() as usize * 2),
            DynamicImage::ImageRgb8(im) => (3, im.width() as usize * 3),
            DynamicImage::ImageRgba8(im) => (4, im.width() as usize * 4),
            DynamicImage::ImageLuma16(im) => (2, im.width() as usize * 2),
            DynamicImage::ImageLumaA16(im) => (4, im.width() as usize * 4),
            DynamicImage::ImageRgb16(im) => (6, im.width() as usize * 6),
            DynamicImage::ImageRgba16(im) => (8, im.width() as usize * 8),
            DynamicImage::ImageRgb32F(im) => (12, im.width() as usize * 12),
            DynamicImage::ImageRgba32F(im) => (16, im.width() as usize * 16),
            _ => unreachable!("Unsupported image format!"),
        };
        let im = im.as_bytes();
        let out = &mut out[..];
        move |blocks: Vec<(usize, usize)>, indices: Vec<usize>, size: usize| {
            let w = size * x_stride;
            blocks.par_iter().enumerate().for_each(|(i, &(y, x))| {
                let (y_, x_) = blocks[indices[i]];
                for v in 0..size {
                    let i = x * x_stride + (y + v) * y_stride;
                    let j = x_ * x_stride + (y_ + v) * y_stride;

                    // SAFETY: Output slice is guaranteed to be non-overlapping
                    #[allow(mutable_transmutes)]
                    unsafe {
                        let out = mem::transmute::<&[u8], &mut [u8]>(out);
                        out[j..j + w].copy_from_slice(&im[i..i + w]);
                    }
                }
            })
        }
    };
    shuffle::shuffle_commands(mask.view_mut(), tile_size, random, f);

    save_buffer(args.output, &out, im.width(), im.height(), im.color())?;

    Ok(())
}
