//! Dělení snímku na dlaždice a detekce změn (dirty rectangles).
//!
//! Optimalizace ze zadání: snímek se dělí na dlaždice 64×64, přenáší se
//! jen změněné (ROI). Porovnání je po bajtech s krokem pro rychlost.

/// Výchozí velikost dlaždice v pixelech.
pub const TILE_PX: u32 = 64;

/// Změněný obdélník (v pixelech).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Porovná dva snímky a vrátí seznam změněných dlaždic.
/// `bpp` = bajtů na pixel (např. 4 pro RGBA).
pub fn diff_tiles(
    prev: &[u8],
    next: &[u8],
    width_px: u32,
    height_px: u32,
    bpp: u32,
    tile_px: u32,
) -> Vec<TileRect> {
    assert_eq!(prev.len(), next.len(), "frame size mismatch");
    let stride = width_px as usize * bpp as usize;
    assert_eq!(
        prev.len(),
        stride * height_px as usize,
        "frame dims mismatch"
    );
    let tile = tile_px.max(1);
    let probe = TileProbe {
        prev,
        next,
        stride,
        bpp: bpp as usize,
    };
    let mut out = Vec::new();
    let mut ty = 0;
    while ty < height_px {
        let mut tx = 0;
        let th = tile.min(height_px - ty);
        while tx < width_px {
            let tw = tile.min(width_px - tx);
            if probe.changed(tx, ty, tw, th) {
                out.push(TileRect {
                    x: tx,
                    y: ty,
                    w: tw,
                    h: th,
                });
            }
            tx += tile;
        }
        ty += tile;
    }
    out
}

/// Porovnávač dlaždic nad dvojicí snímků.
struct TileProbe<'a> {
    prev: &'a [u8],
    next: &'a [u8],
    stride: usize,
    bpp: usize,
}

impl TileProbe<'_> {
    fn changed(&self, tx: u32, ty: u32, tw: u32, th: u32) -> bool {
        // Rychlá cesta: porovnání po 8 bajtech, pak přesný doplatek.
        for row in 0..th as usize {
            let off = (ty as usize + row) * self.stride + tx as usize * self.bpp;
            let len = tw as usize * self.bpp;
            let a = &self.prev[off..off + len];
            let b = &self.next[off..off + len];
            let mut ac = a.chunks_exact(8);
            let mut bc = b.chunks_exact(8);
            if ac.by_ref().ne(bc.by_ref()) || ac.remainder() != bc.remainder() {
                return true;
            }
        }
        false
    }
}

/// Sjednotí sousední obdélníky na stejném řádku dlaždic do širších pásů.
/// Sníží počet paketů; volá se před kódováním.
pub fn merge_row_rects(mut rects: Vec<TileRect>) -> Vec<TileRect> {
    if rects.is_empty() {
        return rects;
    }
    rects.sort_by_key(|r| (r.y, r.x));
    let mut out: Vec<TileRect> = Vec::with_capacity(rects.len());
    for r in rects {
        match out.last_mut() {
            Some(last) if last.y == r.y && last.h == r.h && last.x + last.w == r.x => {
                last.w += r.w;
            }
            _ => out.push(r),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, fill: u8) -> Vec<u8> {
        vec![fill; (w * h * 4) as usize]
    }

    #[test]
    fn identical_frames_have_no_dirty_tiles() {
        let a = frame(128, 128, 0x11);
        let b = a.clone();
        assert!(diff_tiles(&a, &b, 128, 128, 4, TILE_PX).is_empty());
    }

    #[test]
    fn single_pixel_marks_one_tile() {
        let a = frame(128, 128, 0);
        let mut b = a.clone();
        // Pixel (100, 100) = dlaždice (64..128, 64..128).
        let off = (100 * 128 + 100) * 4;
        b[off] = 0xff;
        let dirty = diff_tiles(&a, &b, 128, 128, 4, TILE_PX);
        assert_eq!(
            dirty,
            vec![TileRect {
                x: 64,
                y: 64,
                w: 64,
                h: 64
            }]
        );
    }

    #[test]
    fn partial_edge_tiles() {
        let a = frame(100, 100, 0);
        let mut b = a.clone();
        let off = (99 * 100 + 99) * 4;
        b[off] = 1;
        let dirty = diff_tiles(&a, &b, 100, 100, 4, TILE_PX);
        assert_eq!(
            dirty,
            vec![TileRect {
                x: 64,
                y: 64,
                w: 36,
                h: 36
            }]
        );
    }

    #[test]
    fn merge_coalesces_row_neighbors() {
        let rects = vec![
            TileRect {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            TileRect {
                x: 64,
                y: 0,
                w: 64,
                h: 64,
            },
            TileRect {
                x: 0,
                y: 64,
                w: 64,
                h: 64,
            },
        ];
        let merged = merge_row_rects(rects);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].w, 128);
    }
}
