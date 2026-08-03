/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shape emitters — convert usvg types into backend-agnostic paint commands.
//!
//! Each shape implements the [`Emit`] trait, producing [`PaintCommand`]s
//! that are later consumed by a [`crate::renderer::Renderer`].

pub mod path;
pub mod simple;

use webrender_api::units::LayoutPoint;

/// Backend-agnostic paint command produced by emitters.
#[derive(Debug, Clone)]
pub(crate) enum PaintCommand {
    FillRect {
        bounds: FillRectBounds,
        color: PaintColor,
        clip: Option<RoundedClip>,
    },
    StrokeRect {
        bounds: FillRectBounds,
        color: PaintColor,
        width: f32,
        radii: Option<RoundedRadii>,
    },
    DrawImage {
        x: f32,
        y: f32,
        w: u32,
        h: u32,
        data: Vec<u8>,
        fallback_color: PaintColor,
    },
    StrokeLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: PaintColor,
        width: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FillRectBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PaintColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundedClip {
    pub rx: f32,
    pub ry: f32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundedRadii {
    pub rx: f32,
    pub ry: f32,
}

/// Bundled context passed to every [`Emit::emit`] call.
pub(crate) struct EmitContext {
    pub svg_origin: LayoutPoint,
}

/// Convert an SVG shape into backend-agnostic paint commands.
pub(crate) trait Emit {
    /// Produce paint commands for this shape.
    fn emit(&self, ctx: &EmitContext, commands: &mut Vec<PaintCommand>);
}

// ======================= Helpers =======================

pub(crate) fn color_from_usvg(c: &usvg::Color, opacity: f32) -> PaintColor {
    PaintColor {
        r: c.red as f32 / 255.0,
        g: c.green as f32 / 255.0,
        b: c.blue as f32 / 255.0,
        a: opacity,
    }
}
