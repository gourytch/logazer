use image::{DynamicImage, RgbaImage};
use crate::types::{Meta, Quality};

pub fn parse(image: &DynamicImage) -> Meta {
    
    let rhombus_quality = parse_rhombus_quality(image);
    if rhombus_quality != Quality::Unknown {
        let mut meta = Meta::empty();
        meta.quality = rhombus_quality;
        return meta;
    }
    Meta::empty()
}

//////////////////////////////////////////////////////////////////////////////
/// implementation
//////////////////////////////////////////////////////////////////////////////
use image::{GenericImageView, Rgba};
use epaint::Color32;
use crate::types::{
    COLOR_QUALITY_COMMON,
    COLOR_QUALITY_UNCOMMON,
    COLOR_QUALITY_RARE,
    COLOR_QUALITY_EPIC,
    COLOR_QUALITY_LEGENDARY
};

#[inline]
const fn rgba(c: Color32) -> Rgba<u8> {image::Rgba([c.r(), c.g(), c.b(), 255])}

const COLOR_QUALITY_COMMON_RGBA: Rgba<u8> = rgba(COLOR_QUALITY_COMMON);
const COLOR_QUALITY_UNCOMMON_RGBA: Rgba<u8> = rgba(COLOR_QUALITY_UNCOMMON);
const COLOR_QUALITY_RARE_RGBA: Rgba<u8> = rgba(COLOR_QUALITY_RARE);
const COLOR_QUALITY_EPIC_RGBA: Rgba<u8> = rgba(COLOR_QUALITY_EPIC);
const COLOR_QUALITY_LEGENDARY_RGBA: Rgba<u8> = rgba(COLOR_QUALITY_LEGENDARY);

const DARK_VALUE: u32 = 32;
const DRIFT_VALUE: u32 = 10;

const DARK_THRESHOLD3: u32 = 3 * DARK_VALUE; 
const DRIFT_THRESHOLD3: u32 = 3 * DRIFT_VALUE; 

const COLOR_DARK: Color32 = Color32::from_rgb(DARK_VALUE as u8, DARK_VALUE as u8, DARK_VALUE as u8);

const ERR_RATE_PERCENT_THRESHOLD: u32 = 10;

struct ScreenCoords {
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

const SD_2560_1440: ScreenCoords = ScreenCoords {
    screen_width: 2560,
    screen_height: 1440,
    title_top: 40, // from the top to the white line on the title (~approx)
    title_bottom: 152, // from the top to the line below that is definitely not a title
    title_width: 600, // maximum width for the title
    coords_top: 53,   // from the top to the coords block
    coords_bottom: 74,  // from the top the bottom line of the coords block
    coords_right: 2280, // right boundary of the coords block
    coords_left: 1840,  // left boundary of the coords block
    rhombus_cy: 69,     // y-coord of quality rhombus
    rhombus_size: 25, // diagonal size for the quality rhombus (+-2 px)
};


struct LineMatch {
    pub cx: u32,
    pub cy: u32,
    pub rgba: Rgba<u8>,
    pub quality: Quality,
    pub diff: u32,
}

#[inline]
const fn abs_diff(a: u32, b: u32) -> u32 {
    if a > b { a - b } else { b - a }
}

#[inline]
const fn c_diff(a: u8, b: u8) -> u32 {
    abs_diff(a as u32, b as u32)
}

#[inline]
fn rgba_diff(a: Rgba<u8>, b:Rgba<u8>) -> u32 {
    c_diff(a[0], b[0]) + c_diff(a[1], b[1]) + c_diff(a[2], b[2])
}

#[inline]
fn is_dark(a: Rgba<u8>) -> bool {
    (a[0] as u32 + a[1] as u32 + a[2] as u32) < DARK_THRESHOLD3 ||
        (a[0] < 100) && (a[1] < 100) && (a[2] < 100) // for T0..T4 quality at least one color > 200 and at least one of two left > 100

}

fn best_quality_match(a: Rgba<u8>) -> (Quality, u32) {
    let diffs: [(Quality, u32); 5] = [
        (Quality::Common, rgba_diff(a, COLOR_QUALITY_COMMON_RGBA)),
        (Quality::Uncommon, rgba_diff(a, COLOR_QUALITY_UNCOMMON_RGBA)),
        (Quality::Rare, rgba_diff(a, COLOR_QUALITY_RARE_RGBA)),
        (Quality::Epic, rgba_diff(a, COLOR_QUALITY_EPIC_RGBA)),
        (Quality::Legendary, rgba_diff(a, COLOR_QUALITY_LEGENDARY_RGBA)),
    ];
    diffs.iter().min_by_key(|(_, diff)| *diff).copied().unwrap_or((Quality::Unknown, u32::MAX))
}

fn ddist(image: &DynamicImage, cx: u32, cy: u32, dx: i32, dy: i32, c: Rgba<u8>, max_dist: u32, max_diff: u32) -> u32 {
    let mut x = cx as i32;
    let mut y = cy as i32;
    let mut len = 0;
    loop {
        if max_dist < len {
            return len;
        }
        if max_diff < rgba_diff(c, image.get_pixel(x as u32, y as u32)) {
            return len;
        }
        x += dx;
        y += dy;
        len += 1;
    }
}


fn adjust_center(image: &DynamicImage, sd: &ScreenCoords, cx0: u32, cy0: u32) -> (u32, u32) {
    let mut cx = cx0;
    let mut cy = cy0;

    for _ in 0..4 {
        let c = image.get_pixel(cx, cy);
        let d_left = ddist(image, cx, cy, -1, 0, c, sd.rhombus_size, DRIFT_THRESHOLD3);
        let d_right = ddist(image, cx, cy, 1, 0, c, sd.rhombus_size, DRIFT_THRESHOLD3);
        let cxx = (cx - d_left  + cx + d_right) / 2;
        let d_up = ddist(image, cx, cy, 0, -1, c, sd.rhombus_size, DRIFT_THRESHOLD3);
        let d_down = ddist(image, cx, cy, 0, 1, c, sd.rhombus_size, DRIFT_THRESHOLD3);
        let cyy = (cy - d_up  + cy + d_down) / 2;
        let ok = abs_diff(cx, cxx) < 2 && abs_diff(cy, cyy) < 2;
        cx = cxx;
        cy = cyy;
        if ok { break; }
    }
    (cx, cy)
}



// scanning rectangle, matching with 
//
//    *
//    **
//    * * <- p
// dy *  *
//    *   *
//    ******
//      dx
//
//   p is inside the rhombus when dx + dy <= L/2
//

fn check_rhombus(image: &DynamicImage, sd: &ScreenCoords, lm: &LineMatch) -> bool {
    let d12 = sd.rhombus_size / 2;
    let d38 = sd.rhombus_size * 7 / 16;
    let d32 = sd.rhombus_size * 3 / 4; // the half from 3/2
    let x0 = lm.cx - d32;
    let x1 = lm.cx + d32;
    let y0 = lm.cy - d32;
    let y1 = lm.cy + d32;
    let w = x1 - x0;
    let h = y1 - y0;
    let mut counter = 0;

    // eprintln!("check range x=({};{}) {}x{} ...", x0, x0, w, h);
    // let fname = format!("cropped-{}_{}_{}_{}.png", x0, y0, w, h);
    // let crop = image.crop_imm(x0, y0, w, h);
    // if let Err(err) = crop.save(&fname) {
    //     eprintln!("crop {} not saved: {}", &fname, err);
    // }


    for y in y0..y1 {
        for x in x0..x1 {
            let c = image.get_pixel(x, y);
            let c_dark = is_dark(c);
            let c_match = rgba_diff(c, lm.rgba) < DRIFT_THRESHOLD3;
            let dx = abs_diff(x, lm.cx);
            let dy = abs_diff(y, lm.cy);
            if dx + dy <= d38 {
                // inner zone, color must match
                if !c_match {
                    // eprintln!("({},{}) inner zone mismatch, c={:?}", dx, dy, c);
                    counter += 1;
                }
            } else if dx + dy <= d12 {
                // twilight zone. ignore the color
            } else {
                // outer space, must be dark
                if !c_dark {
                    // eprintln!("({},{}) outer space mismatch, c={:?}", dx, dy, c);
                    counter += 1;
                }
            }
        }
    }
    let err_rate_percent = counter * 100 / ((x1-x0) * (y1-y0));
    let ok = err_rate_percent < ERR_RATE_PERCENT_THRESHOLD;
    // eprintln!("check_rhombus counter={}, err_rate_percent={}, ok={}", counter, err_rate_percent, ok);
    ok
}


// search for line with solid color, before and de after - dark color
fn find_rhombus_line(image: &DynamicImage, sd: &ScreenCoords) -> Option<LineMatch> {
    let (w, h) = image.dimensions();
    if w != sd.screen_width || h != sd.screen_height {
        // eprintln!("dimensions mismatch: {}x{} vs {}x{}", w, h, sd.screen_width, sd.screen_height);
        return None
    }
    let x0 = (w - sd.title_width) / 2; // left boundary for searching
    let x1 = w / 2; // right boundary for searching
    // search for matching pattern: dark - light - dark. dark: (r+g+b) < 33 * 3
    let y = sd.rhombus_cy;
    // eprintln!("find_rhombus_line y={}, x=[{}..{}]", y, x0, x1);
    let mut x_left = x0;
    let mut was_bright = false;
    for x in x0..x1 {
        let c = image.get_pixel(x, y);
        if is_dark(c) {
            // dark pixel
            if was_bright {
                let x_right = x - 1;
                let x_len =  x_right - x_left + 1;
                // eprintln!("   light spot x=[{}..{}], L={})", x_left, x_right, x_len);            
                if abs_diff(sd.rhombus_size, x_len) < 4 {
                    // it seems we found light spot. and it is big enough but not so much
                    // eprintln!("   light spot x=[{}..{}], L={}) has good size", x_left, x_right, x_len);
                    let cx = (x_left + x) / 2;
                    let cm = image.get_pixel(cx, y);
                    // check for solidity
                    let mut s: u32 = 0;
                    let dx = x_len * 7 / 16; // L/2 * ~75%
                    let x0 = cx - dx; // cut out the dirty tails
                    let dd = dx * 2;
                    for i in 0 .. dd {
                        let c = image.get_pixel(x0 + i, y);
                        let d1 = rgba_diff(cm, c);
                        // eprintln!("   [{}] ({};{}) rgba_diff({:?}, {:?}) = {}", i, x0 + i, y, cm, c, d1);
                        s += d1
                    }
                    let d = s / dd;
                    // eprintln!("   ... rgba_diff {}/{}={} vs {}", s, dd, d, DRIFT_THRESHOLD3);
                    if d < DRIFT_THRESHOLD3 { // solid enough
                        let (quality, diff) = best_quality_match(cm);
                        let (adj_x, adj_y) = adjust_center(image, sd, cx, y);
                        // eprintln!("... solid enough, return ({};{}),q={}, diff={}", adj_x, adj_y, quality.to_str(), diff);
                        let lm = LineMatch{
                            cx: adj_x,
                            cy: adj_y,
                            rgba: cm,
                            quality: quality,
                            diff: diff,
                        };
                        // eprintln!("... dig deeper: check for rhombus");
                        if check_rhombus(image, sd, &lm) {
                            // eprintln!("... rhombus check passed! return quality={}", lm.quality.to_str());
                            return Some(lm);
                        }
                    } else {
                        // eprintln!("... dirty");
                    }
                } else {
                    // eprintln!("... size mismatch");
                }
            } else {
                // eprintln!("... in the darkness");
            }
            x_left = x;
            was_bright = false;
        } else {
            // a bright pixel - just move forward
            was_bright = true;
        }
    }
    // eprintln!("found nothing");
    None // did not find anything valuable
}


fn the_best_screen_coords(_image: &DynamicImage) -> ScreenCoords {
    SD_2560_1440 // TODO add more screen definitions later
}


fn parse_rhombus_quality(image: &DynamicImage) -> Quality {
    let sd = the_best_screen_coords(image);
    if let Some(lm) = find_rhombus_line(image, &sd) {
        return lm.quality;
    }
    Quality::Unknown
}

///////////////////////////////////////
/// testbox
//////////////////////////////////////

#[cfg(test)]
mod tests {
    use image::DynamicImage;
    use crate::types::Quality;
    use crate::parser::{parse, parse_rhombus_quality};
    
    fn load_test_png(name: &'static str) -> DynamicImage {
        image::open(format!("./assets/test/{}.png", name)).expect("load error")

    }
    #[test]
    fn test_parse_generic_pix() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_generic_pix"));
        assert_eq!(quality, Quality::Unknown);
    }

    #[test]
    fn test_parse_rhombus_quality_common() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_quality_common"));
        assert_eq!(quality, Quality::Common);
    }

    #[test]
    fn test_parse_quality_uncommon() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_quality_uncommon"));
        assert_eq!(quality, Quality::Uncommon);
    }

    #[test]
    fn test_parse_quality_rare() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_quality_rare"));
        assert_eq!(quality, Quality::Rare);
    }

    #[test]
    fn test_parse_quality_epic() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_quality_epic"));
        assert_eq!(quality, Quality::Epic);
    }
    #[test]
    fn test_parse_quality_legendary() {
        let quality = parse_rhombus_quality(&load_test_png("test_parse_quality_legendary"));
        assert_eq!(quality, Quality::Legendary);
    }
}

