//! A cart as a model: its outline pushed back into a thick slab, drawn by `Draw::Model`.
//!
//! The face is the cart's finished picture laid over a few cells, and the sides are one flat
//! strip for every cut of the outline, each taking its colour from the face just inside its
//! edge, so a cart's colour and finish run on round its sides. A cart is a few dozen triangles.
//!
//! The method, and the outline numbers, are ported from omega-zero by oauramos
//! (https://github.com/oauramos/omega-zero), as Slot is GPL-3.0.

use slot_gfx::{Draw, MeshId, MeshVertex, TexId};

/// How thick a cart is, as a share of its width: a Game Pak is about 8 mm thick and 57 wide.
pub const THICK: f32 = 0.14;

/// The face is laid over this many cells across and down: textures are mapped without
/// perspective, so a face in one piece would bend its label along the diagonal as it turns, the
/// way big polygons did on the PlayStation. A few cells keep a little of that.
const FACE_CELLS: (usize, usize) = (4, 3);

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
pub fn extruded(points: &[(f32, f32)], (uw, uh): (f32, f32), thick: f32) -> Vec<MeshVertex> {
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
                (p.0 - normal[0] * SIDE_SAMPLE) / uw,
                (p.1 - normal[1] * SIDE_SAMPLE) / uh,
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
