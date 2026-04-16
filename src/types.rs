use epaint::Color32;
use log::trace;
use serde::{Deserialize, Serialize};
use std::{fmt, time::Instant};
use image::{DynamicImage, GenericImageView};

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

pub enum EntityGroup {
    Unknown,
    Resources,
    Buildings,
    Containers,
    Bosses,
    Mobs,
    Apes,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[allow(unused)]
pub enum Entity {
    Unknown,
    /* resources */
    Bush, Corn, Poppy, Aloe, Cattail, Cotton, BloodTurnip, BurntBush, VolcanicPlant,
    Pebble, Rock, Sulfur, Obsidian,
    Cactus, CactusTree, Palm, Tree, Driftwood, 
    Bones, RedwoodTree, BurntTree, Mushroom,
    Vine,
    IronOre, GlowingOre, ClayDeposit,
    /* Buildings */
    AncientFabricator,
    /* Containers */
    AmmoBox,
    Urn,
    LootStash,
    Warehouse,
    LargeCase,
    /* Bosses */
    Lazaward,
    ToxicWarPapak,
    WarOkkam,
    Koa,
    Gogo,
    /* Mobs */
    Nurr,
    Phemke,
    /* Apes */
    RupuAshdweller,
    RupuPlainstrider,
    RupuHarraser,
    RupuSeeker,
    RupuScuttler,
    RupuFirebrand,
    RupuDrudge,
    RupuHazraki,
}

impl Entity {
    pub fn to_str(self) -> &'static str {
        match self {
            Entity::Unknown => "Unknown",
            Entity::Bush => "Bush",
            Entity::Driftwood => "Driftwood",
            Entity::Rock => "Rock",
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Unknown" => Some(Quality::Unknown),
            "Common" => Some(Quality::Common),
            "Uncommon" => Some(Quality::Uncommon),
            "Rare" => Some(Quality::Rare),
            "Epic" => Some(Quality::Epic),
            "Legendary" => Some(Quality::Legendary),
            _ => None,
        }
    }
    
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

impl fmt::Display for Quality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
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


//////////////////////////////////////////////////////////////////////////////
/// ScreenCoords
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy)]
pub struct ScreenCoords {
    pub screen_width: u32,
    pub screen_height: u32,
    pub title_top: u32,
    pub title_bottom: u32,
    pub title_width: u32,
    pub coords_top: u32,
    pub coords_bottom: u32,
    pub coords_right: u32,
    pub coords_left: u32,
    pub rhombus_cy: u32,
    pub rhombus_size: u32,
}

pub const SD_BASE: ScreenCoords = ScreenCoords {
    screen_width: 2560,
    screen_height: 1440,
    title_top: 40, // (Ycoord) from the top to the white line on the title (~approx)
    title_bottom: 152, // (Ycoord) from the top to the line below that is definitely not a title
    title_width: 600, // (Xcoord) maximum width for the title
    coords_top: 53,   // (Ycoord) from the top to the coords block
    coords_bottom: 74,  // (Ycoord) from the top the bottom line of the coords block
    coords_right: 2280, // (Xcoord) right boundary of the coords block
    coords_left: 1840,  // (Xcoord) left boundary of the coords block
    rhombus_cy: 69,     // (Ycoord) y-coord of quality rhombus
    rhombus_size: 25, // (Xcoord) diagonal size for the quality rhombus (+-2 px)
};

impl ScreenCoords {
    pub fn scaled(width: u32, height: u32) -> Self {
        macro_rules! scaled_w {
            ($field:ident) => {
                SD_BASE.$field * width / SD_BASE.screen_width
            };
        }
        macro_rules! scaled_h {
            ($field:ident) => {
                SD_BASE.$field * height / SD_BASE.screen_height
            };
        }

        Self {
            screen_width: width,
            screen_height: height,
            title_top: scaled_h!(title_top),
            title_bottom: scaled_h!(title_bottom),
            title_width: scaled_w!(title_width),
            coords_top: scaled_h!(coords_top),
            coords_bottom: scaled_h!(coords_bottom),
            coords_right: scaled_w!(coords_right),
            coords_left: scaled_w!(coords_left),
            rhombus_cy: scaled_h!(rhombus_cy),
            rhombus_size: scaled_w!(rhombus_size),
        }
    }


pub fn get_for_image(image: &DynamicImage) -> ScreenCoords {
    let (width, height) = image.dimensions();    
    if width == SD_BASE.screen_width && height == SD_BASE.screen_height {
        SD_BASE
    } else {
        ScreenCoords::scaled(width, height)
    }
}


}


