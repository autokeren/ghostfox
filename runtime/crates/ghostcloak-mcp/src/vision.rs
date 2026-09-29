//! M2.5 pixel math on compositor-captured RGBA surfaces — runs in OUR
//! process, not in page JS. Nothing here touches the page realm.

/// Downsample an RGBA surface into a compact luminance grid of digits
/// 0-9 (0 = black, 9 = white). Row-major rows of digit chars — the
/// text-model-friendly "superman glasses" view: shapes, holes and
/// orientation readable as numbers.
pub fn luminance_grid(rgba: &[u8], width: u32, height: u32, gw: u32, gh: u32) -> Vec<String> {
    let (w, h) = (width.max(1) as usize, height.max(1) as usize);
    let mut rows = Vec::with_capacity(gh as usize);
    for gy in 0..gh {
        let y0 = gy as usize * h / gh as usize;
        let y1 = ((gy as usize + 1) * h / gh as usize).max(y0 + 1);
        let mut row = String::with_capacity(gw as usize);
        for gx in 0..gw {
            let x0 = gx as usize * w / gw as usize;
            let x1 = ((gx as usize + 1) * w / gw as usize).max(x0 + 1);
            let mut sum: u64 = 0;
            let mut n: u64 = 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = (y * w + x) * 4;
                    if i + 2 >= rgba.len() {
                        continue;
                    }
                    sum += (299 * rgba[i] as u64
                        + 587 * rgba[i + 1] as u64
                        + 114 * rgba[i + 2] as u64)
                        / 1000;
                    n += 1;
                }
            }
            let v = (sum * 9 / n.max(1) + 127) / 255;
            row.push(char::from(b'0' + v as u8));
        }
        rows.push(row);
    }
    rows
}
