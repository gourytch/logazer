use image::{DynamicImage, ImageBuffer, RgbaImage};
use crate::types::{Meta, Quality};


pub fn parse(image: &DynamicImage) -> Meta {
    
    let rhombus_quality = parse_rhombus_quality(image, false).0;
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

// const COLOR_DARK: Color32 = Color32::from_rgb(DARK_VALUE as u8, DARK_VALUE as u8, DARK_VALUE as u8);

const ERR_RATE_PERCENT_THRESHOLD: u32 = 10;


//////////////////////////////////////////////////////////////////////////////
/// ScreenCoords
//////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy)]
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

const SD_BASE: ScreenCoords = ScreenCoords {
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

fn get_scaled_screen_coords(width: u32, height: u32) -> ScreenCoords {
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

    ScreenCoords {
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


fn get_screen_coords(image: &DynamicImage) -> ScreenCoords {
    let (width, height) = image.dimensions();    
    if width == SD_BASE.screen_width && height == SD_BASE.screen_height {
        SD_BASE
    } else {
        get_scaled_screen_coords(width, height)
    }
}

//////////////////////////////////////////////////////////////////////////////
/// LineMatch
//////////////////////////////////////////////////////////////////////////////

struct LineMatch {
    pub cx: u32,
    pub cy: u32,
    pub rgba: Rgba<u8>,
    pub quality: Quality,
    #[allow(unused)] 
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

#[allow(unused)]
#[inline]
fn rgba_diff2(a: Rgba<u8>, b:Rgba<u8>) -> u32 {
    let (dr, dg, db) = (c_diff(a[0], b[0]), c_diff(a[1], b[1]), c_diff(a[2], b[2]));
    dr*dr + dg*dg + db*db
}

#[allow(unused)]
#[inline]
fn is_dark(a: Rgba<u8>) -> bool {
    (a[0] as u32 + a[1] as u32 + a[2] as u32) < DARK_THRESHOLD3 &&
        (a[0] < 100) && (a[1] < 100) && (a[2] < 100) // for T0..T4 quality at least one color > 200 and at least one of two left > 100

}

#[allow(unused)]
#[inline]
fn is_colorful(a: Rgba<u8>) -> bool {
    (a[0] > 200) || (a[1] > 200) || (a[2] > 200) // for T0..T4 quality at least one color > 200 and at least one of two left > 100
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

fn check_rhombus(image: &DynamicImage, sd: &ScreenCoords, lm: &LineMatch, test: bool) -> (bool, Option<RgbaImage>) {
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

    let mut checkmap: Option<RgbaImage> = if test {Some(ImageBuffer::new(w, h))} else {None};

    // eprintln!("check range x=({};{}) {}x{} ...", x0, x0, w, h);
    // let fname = format!("cropped-{}_{}_{}_{}.png", x0, y0, w, h);
    // let crop = image.crop_imm(x0, y0, w, h);
    // if let Err(err) = crop.save(&fname) {
    //     eprintln!("crop {} not saved: {}", &fname, err);
    // }


    for y in 0..h {
        for x in 0..w {
            let xx = x + x0;
            let yy = y + y0;
            let dx = abs_diff(xx, lm.cx);
            let dy = abs_diff(yy, lm.cy);
            let c = image.get_pixel(xx, yy);
            let c_dark = !is_colorful(c); // is_dark(c);
            let c_diff = rgba_diff(c, lm.rgba);
            let c_match = c_diff < DRIFT_THRESHOLD3;
            let ok = if dx + dy <= d38 {
                    // inner zone, color must match
                    // if !c_match {
                    //     eprintln!("inner zone, color {:?} mismatch with {:?} by {}", c, lm.rgba, c_diff);
                    // }
                    c_match
                } else if dx + dy <= d12 {
                    // twilight zone. ignore the color check
                    true
                } else {
                    // outer space, must be dark
                    // if !c_dark {
                    //     eprintln!("outer space, color {:?} should be dark", c);
                    // }
                    c_dark
                };
            if !ok {
                counter += 1;
            }
            if let Some(ref mut canvas) = checkmap {
                let cc: Rgba<u8> = image::Rgba([if ok {c[0]} else {255u8}, if ok {255u8} else {c[1]}, c[2], 255u8]);
                canvas.put_pixel(x, y, cc);
            }
        }
    }
    let err_rate_percent = counter * 100 / ((x1-x0) * (y1-y0));
    let ok = err_rate_percent < ERR_RATE_PERCENT_THRESHOLD;
    // eprintln!("check_rhombus counter={}, err_rate_percent={}, ok={}", counter, err_rate_percent, ok);
    (ok, checkmap)
}


// search for line with solid color, before and de after - dark color
fn find_rhombus_line(image: &DynamicImage, sd: &ScreenCoords, test: bool) -> (Option<LineMatch>, Option<RgbaImage>) {
    let (w, h) = image.dimensions();
    if w != sd.screen_width || h != sd.screen_height {
        // eprintln!("dimensions mismatch: {}x{} vs {}x{}", w, h, sd.screen_width, sd.screen_height);
        return (None, None)
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
        if is_colorful(c) {
            // a bright pixel - check we saw the light and move forward
            was_bright = true;
        } else {
            // dark pixel or any faded color
            if was_bright {
                let x_right = x - 1;
                let x_len =  x_right - x_left + 1;
                // eprintln!("   check light spot x=[{}..{}], L={})", x_left, x_right, x_len);            
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
                        // eprintln!("... line seems solid enough ({};{}),q={}, diff={}", adj_x, adj_y, quality.to_str(), diff);
                        let lm = LineMatch{
                            cx: adj_x,
                            cy: adj_y,
                            rgba: cm,
                            quality: quality,
                            diff: diff,
                        };
                        // eprintln!("... dig deeper: check for rhombus");
                        let (ok, checkmap) = check_rhombus(image, sd, &lm, test);
                        if ok {
                            // eprintln!("... rhombus check passed! return quality={}", lm.quality.to_str());
                            return (Some(lm), checkmap);
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
        }
    }
    // eprintln!("found nothing");
    (None, None) // did not find anything valuable
}

fn visual_report(image: &DynamicImage, checkmap: Option<RgbaImage>) -> DynamicImage {
    let w = image.width() / 2;
    let h = image.height() / 2;
    let mut report: RgbaImage = image.resize_exact(w,h, image::imageops::FilterType::Triangle).into_rgba8();
    if let Some(src) = checkmap {
        let (cw, ch) = (src.width(), src.height());
        const GAP: u32 = 2;
        const BG: Rgba<u8> = Rgba([0u8, 0u8, 0u8, 255u8]);
        // on report surface:
        // - draw black rectangle 0,0,cw+4,ch+4
        // - blitblt checkmap to 2,2, cw2+2,ch+2
        for y in 0 .. (ch + 2 * GAP) {
            for x in 0 .. (cw + 2 * GAP) {
                let is_border = x < GAP || y < GAP || cw + GAP <= x || ch + GAP <= y;
                let c = if is_border {BG} else { *src.get_pixel(x-GAP, y-GAP) };
                report.put_pixel(x, y, c);

            }
        }

    }
    DynamicImage::from(report)
}

fn parse_rhombus_quality(image: &DynamicImage, test: bool) -> (Quality, Option<DynamicImage>) {
    let sd = get_screen_coords(image);
    let (lm, checkmap) = find_rhombus_line(image, &sd, test);
    (
        if let Some(u) = lm { u.quality } else { Quality::Unknown },
        if test { Some(visual_report(image, checkmap)) } else {None}
    )
}

///////////////////////////////////////
/// testbox
//////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use image::DynamicImage;
    use crate::types::Quality;
    use crate::parser::{parse_rhombus_quality};


    #[allow(unused)]
    fn load_image(path: String) -> DynamicImage {
        image::open(path).expect("load error")
    }

    #[allow(unused)]
    fn load_test_png(name: &'static str) -> DynamicImage {
        load_image(format!("./assets/test/{}.png", name))

    }

    #[allow(unused)]
    fn save_report_png(report: &Option<DynamicImage>, name: String) {
        const REPORT_PATH: &'static str = "./tmp/";
        if let Some(src) = report {
            let path = format!("{}/{}.png", REPORT_PATH, name);
            if let Err(err) = fs::create_dir_all(REPORT_PATH) {
                println!("create_dir_all error {}", err);
            } else if let Err(err) = src.save(&path) {
                println!("save error {}", err);
            }
        }
    }


    // check if all the samples in subdir assets/test/Quality/{Quality.to_str()}/
    // has (detected_quality == quality) == outcome
    #[allow(unused)]
    fn batch_diamond_check(quality: Quality, outcome: bool) -> bool {
        let test_name = format!("{}{}", quality.to_str(), if outcome {"+"} else {"-"});
        let test_pathname = format!("./assets/test/batch_diamond_check/{}", test_name);
        let test_dir = Path::new(&test_pathname);
        let mut all_matched = true;
        for entry in fs::read_dir(test_dir).unwrap() {
            let path = entry.unwrap().path();
            let strpath = path.display().to_string();
            let picname = path.file_stem()
                .and_then(|os_str| os_str.to_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let input = load_image(strpath);
            let (reported_quality, report) = parse_rhombus_quality(&input, true);
            let report_name = format!("batch_diamond_check.{}.{}",test_name, picname);
            save_report_png(&report, report_name);
            if (reported_quality == quality) != outcome { all_matched = false; }
        }
        all_matched
    }

    ///// diamond check Quality::Unknown
    #[test]
    fn diamond_check_unknown_positive() {
        assert!(batch_diamond_check(Quality::Unknown, true));
    }

    #[test]
    fn diamond_check_unknown_negative() {
        assert!(batch_diamond_check(Quality::Unknown, false));
    }

    ///// diamond check Quality::Common
    #[test]
    fn diamond_check_common_positive() {
        assert!(batch_diamond_check(Quality::Common, true));
    }

    #[test]
    fn diamond_check_common_negative() {
        assert!(batch_diamond_check(Quality::Common, false));
    }

    ///// diamond check Quality::Uncommon
    #[test]
    fn diamond_check_uncommon_positive() {
        assert!(batch_diamond_check(Quality::Uncommon, true));
    }

    #[test]
    fn diamond_check_uncommon_negative() {
        assert!(batch_diamond_check(Quality::Uncommon, false));
    }

    ///// diamond check Quality::Rare
    #[test]
    fn diamond_check_rare_positive() {
        assert!(batch_diamond_check(Quality::Rare, true));
    }

    #[test]
    fn diamond_check_rare_negative() {
        assert!(batch_diamond_check(Quality::Rare, false));
    }

    ///// diamond check Quality::Epic
    #[test]
    fn diamond_check_epic_positive() {
        assert!(batch_diamond_check(Quality::Epic, true));
    }

    #[test]
    fn diamond_check_epic_negative() {
        assert!(batch_diamond_check(Quality::Epic, false));
    }

    ///// diamond check Quality::Legendary
    #[test]
    fn diamond_check_legendary_positive() {
        assert!(batch_diamond_check(Quality::Legendary, true));
    }

    #[test]
    fn diamond_check_legendary_negative() {
        assert!(batch_diamond_check(Quality::Legendary, false));
    }

}

