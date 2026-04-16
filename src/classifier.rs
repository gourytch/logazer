use std::path::{Path, PathBuf};

use image::{DynamicImage, GrayImage, ImageBuffer, RgbaImage};
use log::trace;
use crate::{
    imagework::{bitline, bitmap},
    types::{Meta, Quality, ScreenCoords}
};

pub struct Classifier {
    basepath: PathBuf,

}

pub struct EntityMeta {
    bitmap: GrayImage,
    bitline: Vec<u8>,
    text: String,
}

impl EntityMeta {
    pub fn from_image(src: &DynamicImage, sc: &ScreenCoords, cx: u32) -> Self {
        Self {
            bitmap: bitmap(src, cx, sc.title_top, sc.title_width, sc.title_bottom - sc.title_top, 254),
            bitline: bitline(src, cx, sc.rhombus_cy, sc.title_width, 254),
            text: "Undefined".to_string(),
        }
    }
}

impl Classifier {
    pub fn new(basepath: impl AsRef<Path>) -> Self {
        Self {
            basepath: basepath.as_ref().to_path_buf(),
        }
    }

}