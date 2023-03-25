use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek};
use std::path::PathBuf;

use anyhow::Error;
use clap::Parser;
use image::io::Reader as ImageReader;
use image::{ColorType, DynamicImage, FlatSamples};
use ndarray::parallel::prelude::*;
use ndarray::prelude::*;

fn read_image<R: Read + BufRead + Seek>(reader: R) -> Result<(Array3<u8>, ColorType), Error> {
    fn from_u8(im: FlatSamples<Vec<u8>>) -> Array3<u8> {
        let mut arr = <Array3<u8>>::zeros((
            im.layout.height as usize,
            im.layout.width as usize,
            im.layout.channels as usize,
        ));
        par_azip!((index (y, x, c), v in &mut arr) {
            *v = im.samples[x * im.layout.width_stride
                + y * im.layout.height_stride
                + c * im.layout.channel_stride];
        });
        arr
    }

    fn from_u16(im: FlatSamples<Vec<u16>>) -> Array3<u8> {
        let mut arr = <Array3<u8>>::zeros((
            im.layout.height as usize,
            im.layout.width as usize,
            im.layout.channels as usize * 2,
        ));

        par_azip!((index (y, x, c), v in &mut arr) {
            *v = im.samples[x * im.layout.width_stride
                + y * im.layout.height_stride
                + (c / 2) * im.layout.channel_stride]
                .to_ne_bytes()[c % 2];
        });
        arr
    }

    fn from_f32(im: FlatSamples<Vec<f32>>) -> Array3<u8> {
        let mut arr = <Array3<u8>>::zeros((
            im.layout.height as usize,
            im.layout.width as usize,
            im.layout.channels as usize * 4,
        ));

        par_azip!((index (y, x, c), v in &mut arr) {
            *v = im.samples[x * im.layout.width_stride
                + y * im.layout.height_stride
                + (c / 4) * im.layout.channel_stride]
                .to_ne_bytes()[c % 4];
        });
        arr
    }

    match ImageReader::new(reader).with_guessed_format()?.decode()? {
        DynamicImage::ImageLuma8(im) => Ok((from_u8(im.into_flat_samples()), ColorType::L8)),
        DynamicImage::ImageLumaA8(im) => Ok((from_u8(im.into_flat_samples()), ColorType::La8)),
        DynamicImage::ImageRgb8(im) => Ok((from_u8(im.into_flat_samples()), ColorType::Rgb8)),
        DynamicImage::ImageRgba8(im) => Ok((from_u8(im.into_flat_samples()), ColorType::Rgba8)),
        DynamicImage::ImageLuma16(im) => Ok((from_u16(im.into_flat_samples()), ColorType::L16)),
        DynamicImage::ImageLumaA16(im) => Ok((from_u16(im.into_flat_samples()), ColorType::La16)),
        DynamicImage::ImageRgb16(im) => Ok((from_u16(im.into_flat_samples()), ColorType::Rgb16)),
        DynamicImage::ImageRgba16(im) => Ok((from_u16(im.into_flat_samples()), ColorType::Rgba16)),
        DynamicImage::ImageRgb32F(im) => Ok((from_f32(im.into_flat_samples()), ColorType::Rgb32F)),
        DynamicImage::ImageRgba32F(im) => {
            Ok((from_f32(im.into_flat_samples()), ColorType::Rgba32F))
        }
        _ => Err(Error::msg("Unsupported image data format")),
    }
}

#[derive(Parser, Debug)]
#[command(author, version)]
struct Args {
    /// Input file
    input: PathBuf,

    /// Mask file
    #[arg(short = 'm', long)]
    mask: Option<PathBuf>,

    /// Output file
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let im = read_image(BufReader::new(File::open(&args.input)?))?;

    Ok(())
}
