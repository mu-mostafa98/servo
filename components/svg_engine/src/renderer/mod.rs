/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Renderer — converts backend-agnostic paint commands into backend-specific API calls.
//!
//! The [`Renderer`] holds a list of [`PaintCommand`]s and dispatches them
//! to a [`Backend`] implementation (WebRender, Krilla, etc.).

pub mod krilla;
pub mod webrender;

use webrender_api::{ClipChainId, SpatialId};

use crate::emitter::PaintCommand;

/// Backend trait — renders paint commands to a specific output target.
pub trait Backend {
    fn fill_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, clip: Option<ClipDesc>,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    );
    fn stroke_rect(
        &mut self, bounds: FillRectDesc, color: PaintColorDesc, width: f32, radii: Option<RadiiDesc>,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    );
    fn stroke_line(
        &mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: PaintColorDesc, width: f32,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    );
    fn draw_image(
        &mut self, x: f32, y: f32, w: u32, h: u32, data: &[u8],
        fallback: PaintColorDesc, spatial_id: SpatialId, clip_chain_id: ClipChainId,
    );
    fn draw_text(
        &mut self, x: f32, y: f32, glyphs: &[TextGlyphDesc],
        font_handle: usize, font_size: f32, color: PaintColorDesc,
        spatial_id: SpatialId, clip_chain_id: ClipChainId,
    );
}

#[derive(Debug, Clone, Copy)]
pub struct TextGlyphDesc {
    pub glyph_id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

pub struct FillRectDesc { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }
pub struct PaintColorDesc { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }
pub struct ClipDesc { pub rx: f32, pub ry: f32 }
pub struct RadiiDesc { pub rx: f32, pub ry: f32 }

/// Collects paint commands and dispatches them to a backend.
pub struct Renderer {
    pub(crate) commands: Vec<PaintCommand>,
}

impl Renderer {
    pub fn new() -> Self {
        Renderer { commands: Vec::new() }
    }

    /// Execute all commands against the given backend.
    pub fn render<B: Backend>(
        &self,
        backend: &mut B,
        spatial_id: SpatialId,
        clip_chain_id: ClipChainId,
    ) {
        for cmd in &self.commands {
            match cmd {
                PaintCommand::FillRect { bounds, color, clip } => {
                    backend.fill_rect(
                        FillRectDesc { x: bounds.x, y: bounds.y, w: bounds.w, h: bounds.h },
                        PaintColorDesc { r: color.r, g: color.g, b: color.b, a: color.a },
                        clip.map(|c| ClipDesc { rx: c.rx, ry: c.ry }),
                        spatial_id, clip_chain_id,
                    );
                }
                PaintCommand::StrokeRect { bounds, color, width, radii } => {
                    backend.stroke_rect(
                        FillRectDesc { x: bounds.x, y: bounds.y, w: bounds.w, h: bounds.h },
                        PaintColorDesc { r: color.r, g: color.g, b: color.b, a: color.a },
                        *width,
                        radii.map(|r| RadiiDesc { rx: r.rx, ry: r.ry }),
                        spatial_id, clip_chain_id,
                    );
                }
                PaintCommand::DrawImage { x, y, w, h, data, fallback_color } => {
                    backend.draw_image(*x, *y, *w, *h, data,
                        PaintColorDesc { r: fallback_color.r, g: fallback_color.g, b: fallback_color.b, a: fallback_color.a },
                        spatial_id, clip_chain_id);
                }
                PaintCommand::StrokeLine { x1, y1, x2, y2, color, width } => {
                    backend.stroke_line(
                        *x1, *y1, *x2, *y2,
                        PaintColorDesc { r: color.r, g: color.g, b: color.b, a: color.a },
                        *width,
                        spatial_id, clip_chain_id,
                    );
                }
                PaintCommand::Text { x, y, glyphs, color, font_handle, font_size } => {
                    let glyph_descs: Vec<TextGlyphDesc> = glyphs.iter().map(|g| {
                        TextGlyphDesc { glyph_id: g.glyph_id, x: g.x, y: g.y, advance: g.advance }
                    }).collect();
                    backend.draw_text(
                        *x, *y, &glyph_descs, *font_handle, *font_size,
                        PaintColorDesc { r: color.r, g: color.g, b: color.b, a: color.a },
                        spatial_id, clip_chain_id,
                    );
                }
            }
        }
    }
}
