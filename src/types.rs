use epaint::Color32;
use log::trace;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use image::{DynamicImage};

pub const NO_COORD: u32 = 9999999;

pub const COLOR_QUALITY_UNKNOWN: Color32 = Color32::from_rgb(128, 128, 128);
pub const COLOR_QUALITY_COMMON: Color32 = Color32::from_rgb(255, 255,255);
pub const COLOR_QUALITY_UNCOMMON: Color32 = Color32::from_rgb(48, 218,120);
pub const COLOR_QUALITY_RARE: Color32 = Color32::from_rgb(0, 113,218);
pub const COLOR_QUALITY_EPIC: Color32 = Color32::from_rgb(218, 64,166);
pub const COLOR_QUALITY_LEGENDARY: Color32 = Color32::from_rgb(218, 159,57);

#[allow(unused)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ViewType {
    Unknown,
    Normal,
    Auger,
}


#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[allow(unused)]
pub enum Entity {
    Unknown,
    Bush,
    Wood,
    Stone,
}

impl Entity {
    pub fn to_str(self) -> &'static str {
        match self {
            Entity::Unknown => "Unknown",
            Entity::Bush => "Bush",
            Entity::Wood => "Wood",
            Entity::Stone => "Stone",
            #[allow(unused)]
            _ => "Unhandled",
        }
    }
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Serialize, Deserialize, Debug)]
pub enum Quality {
    Unknown,
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary
}

impl Quality {
    pub fn to_str(self) -> &'static str {
        match self {
            Quality::Unknown => "Unknown",
            Quality::Common => "Common",
            Quality::Uncommon => "Uncommon",
            Quality::Rare => "Rare",
            Quality::Epic => "Epic",
            Quality::Legendary => "Legendary",
            // _ => "Unhandled",
        }
    }

    pub fn to_color32(self) -> Color32 {
        match self {
            Quality::Unknown => COLOR_QUALITY_UNKNOWN,
            Quality::Common => COLOR_QUALITY_COMMON,
            Quality::Uncommon => COLOR_QUALITY_UNCOMMON,
            Quality::Rare => COLOR_QUALITY_RARE,
            Quality::Epic => COLOR_QUALITY_EPIC,
            Quality::Legendary => COLOR_QUALITY_LEGENDARY,
            // _ => "Unhandled",
        }
    }


}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Meta {
    pub quality: Quality,
    pub entity: Entity,
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub a: u32,
}


impl Meta {
    pub fn empty() -> Self {
        Self {
            quality: Quality::Unknown,
            entity: Entity::Unknown,
            x: NO_COORD,
            y: NO_COORD,
            z: NO_COORD,
            a: NO_COORD,
        }
    }

    pub fn same(&self, other: &Self) -> bool {
        (self.quality == other.quality) && (self.entity == other.entity)
    }

    #[allow(unused)]
    pub fn to_str(&self) -> String {
        format!("(quality:{}, entity:'{}')", self.quality.to_str(), &self.entity.to_str())
    }
}


#[derive(Debug)]
pub struct Screenshot {
    pub pit_captured: Instant, // point-in-time when captured
    pub pit_received: Option<Instant>, // point-in-time when received by processor
    pub pit_parsed: Option<Instant>, // point-in-time when parsed by parser
    pub image: DynamicImage,
    pub meta: Meta,
}

impl Screenshot {
    pub fn new(image: DynamicImage) -> Self {
         Self {
            pit_captured: Instant::now(),
            pit_received: None,
            pit_parsed: None,
            image: image,
            meta: Meta::empty(),            
        }
    }

    pub fn set_received(&mut self) {
        let t = Instant::now();
        trace!("received in {:?}", t.duration_since(self.pit_captured));
        self.pit_received = Some(t);
    }

    pub fn set_parsed(&mut self) {
        let t = Instant::now();
        if let Some(p) = self.pit_received {
            trace!("parsed in {:?}", t.duration_since(p));
        }
        self.pit_parsed = Some(t);
    }
}