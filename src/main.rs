//! Main Program for Jigshuffle
//! Run with `--help` for more instruction

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

mod shuffle;

use std::fs::File;
use std::io::BufReader;
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

    /// Rotate blocks randomly
    #[arg(long)]
    rotate: bool,

    /// Flip blocks randomly
    #[arg(long)]
    flip: bool,

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

    let config = shuffle::ConfigBuilder::new()
        .chunk_po2(tile_size)
        .rotate(args.rotate)
        .flip(args.flip)
        .build();

    let mut random = if let Some(seed) = args.seed {
        let mut hasher = Sha256::new();
        hasher.update(seed);

        rand_xoshiro::Xoshiro256StarStar::from_seed(hasher.finalize().into())
    } else {
        rand_xoshiro::Xoshiro256StarStar::from_entropy()
    };

    let im = ImageReader::new(BufReader::new(File::open(&args.input)?))
        .with_guessed_format()?
        .decode()?;

    let mask = if let Some(p) = args.mask {
        let mi = ImageReader::new(BufReader::new(File::open(p)?))
            .with_guessed_format()?
            .decode()?
            .into_luma8();

        let mut arr = <Array2<bool>>::default((im.height() as usize, im.width() as usize));
        par_azip!((index (y, x), a in &mut arr) {
            *a = match mi.get_pixel_checked(x as _, y as _) {
                None => false,
                Some(Luma([v])) if *v >= 254 => false,
                _ => true,
            };
        });
        arr
    } else {
        <Array2<bool>>::from_elem((im.height() as usize, im.width() as usize), true)
    };

    let arr = <ArrayView3<u8>>::from_shape(
        (
            im.height() as usize,
            im.width() as usize,
            match im {
                DynamicImage::ImageLuma8(_) => 1,
                DynamicImage::ImageLumaA8(_) => 2,
                DynamicImage::ImageRgb8(_) => 3,
                DynamicImage::ImageRgba8(_) => 4,
                DynamicImage::ImageLuma16(_) => 2,
                DynamicImage::ImageLumaA16(_) => 4,
                DynamicImage::ImageRgb16(_) => 6,
                DynamicImage::ImageRgba16(_) => 8,
                DynamicImage::ImageRgb32F(_) => 12,
                DynamicImage::ImageRgba32F(_) => 16,
                _ => unreachable!("Unsupported image format"),
            },
        ),
        im.as_bytes(),
    )?;

    let out = shuffle::jigshuffle(arr, mask.view(), &config, &mut random);

    save_buffer(
        args.output,
        out.as_slice().expect("Should be standard-layout"),
        im.width(),
        im.height(),
        im.color(),
    )?;

    Ok(())
}
