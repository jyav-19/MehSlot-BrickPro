//! A cart as a model: its outline pushed back into a thick slab, drawn by `Draw::Model`.
//!
//! The face is the cart's finished picture laid over a few cells, and the sides are one flat
//! strip for every cut of the outline, each taking its colour from the face just inside its
//! edge, so a cart's colour and finish run on round its sides. A cart is a few dozen triangles.
//!
//! The method, and the outline numbers, are ported from omega-zero by oauramos
//! (https://github.com/oauramos/omega-zero), as Slot is GPL-3.0.

use std::sync::OnceLock;

use slot_gfx::{Draw, MeshId, MeshVertex, TexId};

use crate::cart::{GB_CART_H, GB_CART_W, NES_CART_H, NES_CART_W, SNES_CART_H, SNES_CART_W};
use crate::silhouette::{gb_cart_mask, nes_cart_mask, snes_cart_mask, GbShell, NesShell, SnesShell};

/// How thick a cart is, as a share of its width: a Game Pak is about 8 mm thick and 57 wide.
pub const THICK: f32 = 0.14;

/// The face is laid over this many cells across and down: textures are mapped without
/// perspective, so a face in one piece would bend its label along the diagonal as it turns, the
/// way big polygons did on the PlayStation. A few cells keep a little of that.
// omega-zero uses (4, 3), which bends a label as the cart turns; more cells keep it straight.
const FACE_CELLS: (usize, usize) = (16, 12);

/// How far in from the outline, in drawing units, a side takes its colour from the face.
const SIDE_SAMPLE: f32 = 9.0;

/// How much light the back edge of a side gets against its front edge.
const SIDE_BACK_SHADE: f32 = 0.72;

/// A GBA cart's outline in the units of its drawing (240 by 141), right half, going round
/// clockwise from the top: the grip's corners cut in three, the step down to the body, and the
/// body's own corners cut in two. The left half is its mirror.
const GBA_HALF: &[(f32, f32)] = &[
    (229.3, 0.0),
    (234.9, 2.2),
    (238.8, 7.4),
    (240.0, 11.85),
    (240.0, 25.6),
    (237.6, 28.6),
    (233.5, 28.89),
    (233.5, 136.7),
    (232.2, 139.4),
    (229.0, 141.0),
];

/// The GBA cart's outline in drawing units, all the way round clockwise, and the size of the
/// drawing.
pub fn gba_outline() -> (Vec<(f32, f32)>, (f32, f32)) {
    let mut all: Vec<(f32, f32)> = GBA_HALF.to_vec();
    all.extend(GBA_HALF.iter().rev().map(|&(x, y)| (240.0 - x, y)));
    (all, (240.0, 141.0))
}

/// The GBA cart's model, ready to hand to the GPU once.
pub fn gba_mesh() -> Vec<MeshVertex> {
    let (points, units) = gba_outline();
    extruded(&points, units, THICK)
}

/// A cart in few polygons: its face, the whole texture laid over a few cells, whose outline
/// the texture's own alpha cuts; and its sides, one flat polygon for each cut of its outline
/// `points` (in drawing units, the drawing `uw` by `uh`), back to `thick`, each taking its
/// colour from the face just inside its edge. Sides come first, so the face covers them.
pub fn extruded(points: &[(f32, f32)], units: (f32, f32), thick: f32) -> Vec<MeshVertex> {
    extruded_sampled(points, units, thick, SIDE_SAMPLE)
}

/// [`extruded`], its sides taking their colour from `reach` drawing units inside the edge.
pub fn extruded_sampled(
    points: &[(f32, f32)],
    (uw, uh): (f32, f32),
    thick: f32,
    reach: f32,
) -> Vec<MeshVertex> {
    let mut out = Vec::new();
    for (i, &a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            continue;
        }
        let normal = [dy / len, -dx / len, 0.0];
        let sample = |p: (f32, f32)| {
            [
                (p.0 - normal[0] * reach) / uw,
                (p.1 - normal[1] * reach) / uh,
            ]
        };
        let corner = |p: (f32, f32), z: f32, shade: f32| MeshVertex {
            xyz: [p.0 / uw, p.1 / uh, z],
            normal,
            uv: sample(p),
            shade,
        };
        let (a0, b0) = (corner(a, 0.0, 1.0), corner(b, 0.0, 1.0));
        let (a1, b1) = (
            corner(a, thick, SIDE_BACK_SHADE),
            corner(b, thick, SIDE_BACK_SHADE),
        );
        out.extend([a0, b0, b1, a0, b1, a1]);
    }
    let (cols, rows) = FACE_CELLS;
    let at = |c: usize, r: usize| {
        let (u, v) = (c as f32 / cols as f32, r as f32 / rows as f32);
        MeshVertex {
            xyz: [u, v, 0.0],
            normal: [0.0, 0.0, -1.0],
            uv: [u, v],
            shade: 1.0,
        }
    };
    for r in 0..rows {
        for c in 0..cols {
            let (p, q, s, t) = (at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1));
            out.extend([p, q, s, p, s, t]);
        }
    }
    out
}

/// A cart at `x, y, w, h` as its model, `mesh`, textured with its face, tinted `colour`, turned
/// `turn` radians in the canvas and spun `yaw` about its upright axis.
pub fn posed_cart(
    (x, y, w, h): (f32, f32, f32, f32),
    mesh: MeshId,
    tex: TexId,
    colour: [f32; 4],
    turn: f32,
    yaw: f32,
) -> Draw {
    Draw::Model {
        x,
        y,
        w,
        h,
        mesh,
        tex,
        colour,
        turn,
        yaw,
    }
}

/// The cart shapes that have a model: each is a cart's outline pushed back into a slab.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Kind {
    Gba,
    GbNotched,
    GbRounded,
    Nes,
    Famicom,
    SnesUs,
    SnesPal,
    Disc,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Gba,
        Kind::GbNotched,
        Kind::GbRounded,
        Kind::Nes,
        Kind::Famicom,
        Kind::SnesUs,
        Kind::SnesPal,
        Kind::Disc,
    ];

    fn index(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// How thick the NES, Famicom and SNES carts are, as a share of their width (about; the Game
/// Pak's is `THICK`).
const NES_THICK: f32 = 0.15;
const FAMICOM_THICK: f32 = 0.14;
const SNES_THICK: f32 = 0.15;

/// A cart's model, ready to hand to the GPU once. The GBA's outline is written out; the others
/// are traced from the cart's own silhouette, so a side always meets the edge of its face.
pub fn mesh_vertices(kind: Kind) -> Vec<MeshVertex> {
    let traced = |mask: &[u8], w: u32, h: u32, thick: f32| {
        let (points, units) = traced_outline(mask, w as usize, h as usize);
        extruded_sampled(&points, units, thick, SIDE_SAMPLE * units.0 / 240.0)
    };
    match kind {
        Kind::Gba => gba_mesh(),
        Kind::GbNotched => traced(gb_cart_mask(GbShell::Notched), GB_CART_W, GB_CART_H, THICK),
        Kind::GbRounded => traced(gb_cart_mask(GbShell::Rounded), GB_CART_W, GB_CART_H, THICK),
        Kind::Nes => traced(nes_cart_mask(NesShell::Nes), NES_CART_W, NES_CART_H, NES_THICK),
        Kind::Famicom => traced(
            nes_cart_mask(NesShell::Famicom),
            NES_CART_W,
            NES_CART_H,
            FAMICOM_THICK,
        ),
        Kind::SnesUs => traced(snes_cart_mask(SnesShell::Us), SNES_CART_W, SNES_CART_H, SNES_THICK),
        Kind::SnesPal => traced(
            snes_cart_mask(SnesShell::Pal),
            SNES_CART_W,
            SNES_CART_H,
            SNES_THICK,
        ),
        Kind::Disc => disc_mesh(),
    }
}

/// How thick a disc is, as a share of its width (a real one is 1 mm in 120, too thin to see
/// the side of; this one is thicker so its edge shows as it sways), and how many cuts its
/// round outline has.
const DISC_THICK: f32 = 0.03;
const DISC_SEGMENTS: usize = 48;

/// A disc: a round outline pushed back a little. The hole and the see-through hub are cut by
/// the disc's face itself, as a cart's outline is.
fn disc_mesh() -> Vec<MeshVertex> {
    let n = crate::disc::DISC_PX as f32;
    let r = n / 2.0;
    let points: Vec<(f32, f32)> = (0..DISC_SEGMENTS)
        .map(|i| {
            let turn = std::f32::consts::TAU * i as f32 / DISC_SEGMENTS as f32;
            let a = turn - std::f32::consts::FRAC_PI_2;
            (r + r * a.cos(), r + r * a.sin())
        })
        .collect();
    extruded_sampled(&points, (n, n), DISC_THICK, SIDE_SAMPLE * n / 240.0)
}

/// A silhouette `w` by `h` (coverage, opaque inside) as an outline going round clockwise, in
/// pixels, with the size of the picture: the left and right edge of every row, with the rows
/// that run straight cut down to their corners.
pub fn traced_outline(mask: &[u8], w: usize, h: usize) -> (Vec<(f32, f32)>, (f32, f32)) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for y in 0..h {
        let Some(row) = mask.get(y * w..(y + 1) * w) else {
            break;
        };
        let Some(first) = row.iter().position(|&a| a > 127) else {
            continue;
        };
        let last = row.iter().rposition(|&a| a > 127).unwrap_or(first);
        let mid = y as f32 + 0.5;
        left.push((first as f32, mid));
        right.push((last as f32 + 1.0, mid));
    }
    let size = (w as f32, h as f32);
    let (Some(&(l0, _)), Some(&(r0, _)), Some(&(ln, _)), Some(&(rn, _))) =
        (left.first(), right.first(), left.last(), right.last())
    else {
        return (vec![(0.0, 0.0), (size.0, 0.0), (size.0, size.1), (0.0, size.1)], size);
    };
    let mut down = vec![(r0, 0.0)];
    down.extend(right.iter().copied());
    down.push((rn, size.1));
    let mut up = vec![(ln, size.1)];
    up.extend(left.iter().rev().copied());
    up.push((l0, 0.0));
    let mut all = simplify(&down, TRACE_TOLERANCE);
    all.extend(simplify(&up, TRACE_TOLERANCE));
    (all, size)
}

/// How far, in pixels, a traced outline may stray from the silhouette it follows.
const TRACE_TOLERANCE: f32 = 0.8;

/// `points`, a line, with the points that lie within `tolerance` of the line between their
/// neighbours left out (Ramer-Douglas-Peucker).
fn simplify(points: &[(f32, f32)], tolerance: f32) -> Vec<(f32, f32)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let (a, b) = (points[0], points[points.len() - 1]);
    let (mut far, mut at) = (0.0f32, 0usize);
    for (i, &p) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let d = distance_to_line(p, a, b);
        if d > far {
            far = d;
            at = i;
        }
    }
    if far <= tolerance {
        return vec![a, b];
    }
    let mut first = simplify(&points[..=at], tolerance);
    let second = simplify(&points[at..], tolerance);
    first.pop();
    first.extend(second);
    first
}

fn distance_to_line(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-3 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
}

/// Every cart model once it is on the GPU, set once by the frontend, in the order of
/// [`Kind::ALL`].
static MESHES: OnceLock<[MeshId; 8]> = OnceLock::new();

pub fn set_meshes(ids: [MeshId; 8]) {
    let _ = MESHES.set(ids);
}

/// The model of `kind`, if the frontend has put it on the GPU: without it carts are drawn flat.
pub fn mesh_of(kind: Kind) -> Option<MeshId> {
    MESHES.get().map(|ids| ids[kind.index()])
}
