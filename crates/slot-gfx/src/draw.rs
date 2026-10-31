use crate::quad::Quad;
use crate::shaders::{MODEL_FRAG, MODEL_VERT, SPRITE_FRAG, SPRITE_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TexId(usize);

#[cfg(feature = "test-support")]
impl TexId {
    pub const fn from_raw(id: usize) -> Self {
        TexId(id)
    }
}

/// How far in front of the canvas the eye stands when it looks at a model, in canvas pixels:
/// the nearer it stands, the more the side of a model that turns towards it grows.
pub const FOCAL: f32 = 650.0;

/// Where the point `dx` across from the upright middle of a face lands, measured across the
/// canvas from that middle, once the face is spun `yaw` and slid back `depth`.
pub fn spun_x(dx: f32, yaw: f32, depth: f32) -> f32 {
    let (sin, cos) = yaw.sin_cos();
    (dx * cos - depth * sin) * FOCAL / (FOCAL + dx * sin)
}

/// A model handed to the GPU once, drawn by `Draw::Model`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct MeshId(usize);

/// A corner of one of a model's triangles: `xyz` across and down its rect as shares of the
/// rect's size, and back from its face as a share of its width; `normal` the way the polygon
/// faces, x right, y down and z away from the eye; `uv` where it takes its texture from; and
/// `shade`, how much of the light falls on it.
#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(C)]
pub struct MeshVertex {
    pub xyz: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub shade: f32,
}

/// How models are drawn. Every vertex lands on a grid of `MODEL_SNAP` canvas pixels, the colour
/// is cut to 15 bits through a 4x4 dither of `MODEL_DITHER`-pixel cells, and the light is a
/// point `MODEL_LIGHT` canvas pixels from a model's middle, up, to the left and in front of it,
/// with `MODEL_AMBIENT` of it falling on every side alike. (These are omega-zero's values, so
/// the look matches it; they are the knobs to turn for a smoother one.)
// omega-zero's value is 2.0, a PlayStation-style grid that makes a turning model shimmer.
const MODEL_SNAP: f32 = 0.125;
const MODEL_DITHER: f32 = 2.0;
const MODEL_LIGHT: [f32; 3] = [-700.0, -500.0, -900.0];
const MODEL_AMBIENT: f32 = 0.68;

const XYZ_LOCATION: gl::types::GLuint = 0;
const NORMAL_LOCATION: gl::types::GLuint = 1;
const UVS_LOCATION: gl::types::GLuint = 2;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Draw {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        colour: [f32; 4],
    },
    Tex {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
    },
    Turned {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        turn: f32,
    },
    /// `mesh` laid out over the rect at `x, y, w, h`, textured with `tex` tinted `colour`,
    /// turned `turn` radians in the canvas and spun `yaw` radians about its upright middle.
    Model {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        mesh: MeshId,
        tex: TexId,
        colour: [f32; 4],
        turn: f32,
        yaw: f32,
    },
    Game,
    Shot {
        tex: TexId,
    },
}

/// The program models are drawn with, and where its uniforms are.
struct ModelProgram {
    prog: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_turn: gl::types::GLint,
    u_spin: gl::types::GLint,
}

impl ModelProgram {
    fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program_with(
            MODEL_VERT,
            MODEL_FRAG,
            &[
                ("a_xyz", XYZ_LOCATION),
                ("a_normal", NORMAL_LOCATION),
                ("a_uvs", UVS_LOCATION),
            ],
        )?;
        unsafe {
            gl::UseProgram(prog);
            let at = |name: &str| crate::gl::uniform_location(prog, name);
            gl::Uniform1i(at("u_tex"), 0);
            gl::Uniform2f(at("u_target"), OUT_W as f32, OUT_H as f32);
            gl::Uniform1f(at("u_focal"), FOCAL);
            let [lx, ly, lz] = MODEL_LIGHT;
            gl::Uniform4f(at("u_light"), lx, ly, lz, MODEL_AMBIENT);
            gl::Uniform1f(at("u_snap"), MODEL_SNAP);
            gl::Uniform1f(at("u_dither"), MODEL_DITHER);
            Ok(ModelProgram {
                prog,
                u_rect: at("u_rect"),
                u_colour: at("u_colour"),
                u_turn: at("u_turn"),
                u_spin: at("u_spin"),
            })
        }
    }
}

/// A model's triangles on the GPU.
struct Mesh {
    vao: gl::types::GLuint,
    vbo: gl::types::GLuint,
    count: i32,
}

/// Points the model attributes at the bound vertex buffer, laid out as `MeshVertex`.
unsafe fn mesh_attributes() {
    let stride = std::mem::size_of::<MeshVertex>() as i32;
    let at = |floats: usize| (floats * std::mem::size_of::<f32>()) as *const std::ffi::c_void;
    for (location, size, from) in [
        (XYZ_LOCATION, 3, 0),
        (NORMAL_LOCATION, 3, 3),
        (UVS_LOCATION, 3, 6),
    ] {
        gl::EnableVertexAttribArray(location);
        gl::VertexAttribPointer(location, size, gl::FLOAT, gl::FALSE, stride, at(from));
    }
}

pub struct Sprites {
    prog: gl::types::GLuint,
    white: gl::types::GLuint,
    textures: Vec<gl::types::GLuint>,
    u_rect: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_turn: gl::types::GLint,
    /// Models; none if their shader would not build here, and then they are simply not drawn.
    model: Option<ModelProgram>,
    meshes: Vec<Mesh>,
}

impl Sprites {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(SPRITE_VERT, SPRITE_FRAG)?;
        let white = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[255u8; 4]),
        );
        let (u_rect, u_colour, u_turn);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_colour = crate::gl::uniform_location(prog, "u_colour");
            u_turn = crate::gl::uniform_location(prog, "u_turn");
        }
        let model = match ModelProgram::new() {
            Ok(model) => Some(model),
            Err(e) => {
                eprintln!("slot: models: shader did not build, carts stay flat: {e:?}");
                None
            }
        };
        Ok(Sprites {
            prog,
            white,
            textures: Vec::new(),
            u_rect,
            u_colour,
            u_turn,
            model,
            meshes: Vec::new(),
        })
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::LINEAR)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::NEAREST)
    }

    fn push(&mut self, w: u32, h: u32, rgba: &[u8], filter: gl::types::GLenum) -> TexId {
        let tex = crate::gl::texture(w, h, filter, gl::CLAMP_TO_EDGE, gl::RGBA, Some(rgba));
        self.textures.push(tex);
        TexId(self.textures.len() - 1)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        let Some(tex) = self.textures.get(id.0) else {
            return;
        };
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                crate::gl::internal_format(gl::RGBA),
                w as i32,
                h as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    /// Hands a model's triangles, three corners each, to the GPU.
    pub fn create_mesh(&mut self, vertices: &[MeshVertex]) -> MeshId {
        unsafe {
            let mut vao = 0;
            if !crate::gl::es() {
                gl::GenVertexArrays(1, &mut vao);
                gl::BindVertexArray(vao);
            }
            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                std::mem::size_of_val(vertices) as isize,
                vertices.as_ptr() as *const std::ffi::c_void,
                gl::STATIC_DRAW,
            );
            if !crate::gl::es() {
                mesh_attributes();
                gl::BindVertexArray(0);
            }
            self.meshes.push(Mesh {
                vao,
                vbo,
                count: vertices.len() as i32,
            });
        }
        MeshId(self.meshes.len() - 1)
    }

    /// Draws one model, and leaves the sprite program to be picked again.
    fn draw_model(&self, item: &Draw) {
        let Draw::Model {
            x,
            y,
            w,
            h,
            mesh,
            tex,
            colour,
            turn,
            yaw,
        } = *item
        else {
            return;
        };
        let Some(program) = self.model.as_ref() else {
            return;
        };
        let (Some(mesh), Some(tex)) = (self.meshes.get(mesh.0), self.textures.get(tex.0)) else {
            return;
        };
        unsafe {
            gl::UseProgram(program.prog);
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::Uniform4f(program.u_rect, x, y, w, h);
            gl::Uniform2f(program.u_turn, turn.cos(), turn.sin());
            let (yaw_sin, yaw_cos) = yaw.sin_cos();
            gl::Uniform3f(program.u_spin, yaw_sin, yaw_cos, 0.0);
            gl::Uniform4f(program.u_colour, colour[0], colour[1], colour[2], colour[3]);
            if crate::gl::es() {
                gl::BindBuffer(gl::ARRAY_BUFFER, mesh.vbo);
                mesh_attributes();
            } else {
                gl::BindVertexArray(mesh.vao);
            }
            gl::DrawArrays(gl::TRIANGLES, 0, mesh.count);
            if crate::gl::es() {
                gl::DisableVertexAttribArray(NORMAL_LOCATION);
                gl::DisableVertexAttribArray(UVS_LOCATION);
            } else {
                gl::BindVertexArray(0);
            }
        }
    }

    pub fn source(&self, id: TexId) -> Option<gl::types::GLuint> {
        self.textures.get(id.0).copied()
    }

    pub fn draw(&self, items: &[Draw], quad: &Quad) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        for item in items {
            if let Draw::Model { .. } = item {
                self.draw_model(item);
                unsafe {
                    gl::UseProgram(self.prog);
                    gl::ActiveTexture(gl::TEXTURE0);
                }
                continue;
            }
            let (x, y, w, h, tex, colour, turn) = match *item {
                Draw::Rect { x, y, w, h, colour } => (x, y, w, h, self.white, colour, 0.0),
                Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], 0.0),
                    None => continue,
                },
                Draw::Turned {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    turn,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], turn),
                    None => continue,
                },
                Draw::Model { .. } | Draw::Game | Draw::Shot { .. } => continue,
            };
            let (cos, sin) = if turn == 0.0 {
                (1.0, 0.0)
            } else {
                (turn.cos(), turn.sin())
            };
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::Uniform4f(self.u_rect, x, y, w, h);
                gl::Uniform2f(self.u_turn, cos, sin);
                gl::Uniform4f(self.u_colour, colour[0], colour[1], colour[2], colour[3]);
            }
            quad.draw();
        }
        unsafe { gl::Disable(gl::BLEND) };
    }
}

impl Drop for Sprites {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
            gl::DeleteTextures(1, &self.white);
            gl::DeleteProgram(self.prog);
            if let Some(model) = &self.model {
                gl::DeleteProgram(model.prog);
            }
            for mesh in &self.meshes {
                gl::DeleteBuffers(1, &mesh.vbo);
                if mesh.vao != 0 {
                    gl::DeleteVertexArrays(1, &mesh.vao);
                }
            }
        }
    }
}
