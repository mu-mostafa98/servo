use lyon::math::Point as LyonPoint;
use lyon::tessellation::{FillTessellator, FillOptions, FillVertex, FillVertexConstructor, VertexBuffers, BuffersBuilder};
use lyon::path::polygon::Polygon;
use webrender_api::{
    ClipChainId, CommonItemProperties, DisplayListBuilder, SpaceAndClipInfo, SpatialId,
    units::{LayoutPoint, LayoutRect, LayoutSize},
};

use crate::style::FillRule;

struct PosCtor;

impl FillVertexConstructor<LyonPoint> for PosCtor {
    fn new_vertex(&mut self, vertex: FillVertex) -> LyonPoint {
        vertex.position()
    }
}

pub fn tessellate_polygon(
    points: &[LyonPoint],
    fill_rule: FillRule,
    color: webrender_api::ColorF,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    if points.len() < 3 {
        return;
    }

    let polygon = Polygon { points, closed: true };
    let mut tessellator = FillTessellator::new();
    let mut buffers: VertexBuffers<LyonPoint, u32> = VertexBuffers::new();

    let lyon_fill_rule = match fill_rule {
        FillRule::NonZero => lyon::path::FillRule::NonZero,
        FillRule::EvenOdd => lyon::path::FillRule::EvenOdd,
    };

    if tessellator
        .tessellate(
            polygon.path_events(),
            &FillOptions::default()
                .with_fill_rule(lyon_fill_rule)
                .with_tolerance(0.01),
            &mut BuffersBuilder::new(&mut buffers, PosCtor),
        )
        .is_err()
    {
        return;
    }

    for tri in buffers.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = buffers.vertices[tri[0] as usize];
        let v1 = buffers.vertices[tri[1] as usize];
        let v2 = buffers.vertices[tri[2] as usize];

        scanline_fill_triangle(v0, v1, v2, color, spatial_id, clip_chain_id, wr);
    }
}

fn scanline_fill_triangle(
    v0: LyonPoint,
    v1: LyonPoint,
    v2: LyonPoint,
    color: webrender_api::ColorF,
    spatial_id: SpatialId,
    clip_chain_id: ClipChainId,
    wr: &mut DisplayListBuilder,
) {
    let (top, mid, bot) = sort_vertices_by_y(v0, v1, v2);

    let top_y = top.y.ceil();
    let bot_y = bot.y.floor();
    if top_y > bot_y {
        return;
    }

    let inv_dy_tm = if mid.y != top.y { 1.0 / (mid.y - top.y) } else { 0.0 };
    let inv_dy_tb = if bot.y != top.y { 1.0 / (bot.y - top.y) } else { 0.0 };
    let inv_dy_mb = if bot.y != mid.y { 1.0 / (bot.y - mid.y) } else { 0.0 };

    let dx_tm = (mid.x - top.x) * inv_dy_tm;
    let dx_tb = (bot.x - top.x) * inv_dy_tb;
    let dx_mb = (bot.x - mid.x) * inv_dy_mb;

    let y_start = top_y as i32;
    let y_end = bot_y as i32;

    for y in y_start..=y_end {
        let yf = y as f32;
        let center = yf + 0.5;

        let (x_left, x_right) = if center < mid.y {
            let x_edge_a = top.x + dx_tm * (center - top.y);
            let x_edge_b = top.x + dx_tb * (center - top.y);
            (x_edge_a.min(x_edge_b), x_edge_a.max(x_edge_b))
        } else {
            let x_edge_a = top.x + dx_tb * (center - top.y);
            let x_edge_b = mid.x + dx_mb * (center - mid.y);
            (x_edge_a.min(x_edge_b), x_edge_a.max(x_edge_b))
        };

        let width = x_right - x_left;
        if width > 0.0 {
            let rect = LayoutRect::from_origin_and_size(
                LayoutPoint::new(x_left, yf),
                LayoutSize::new(width, 1.0),
            );
            let common = CommonItemProperties::new(
                rect,
                SpaceAndClipInfo { spatial_id, clip_chain_id },
            );
            wr.push_rect(&common, rect, color);
        }
    }
}

fn sort_vertices_by_y(
    a: LyonPoint,
    b: LyonPoint,
    c: LyonPoint,
) -> (LyonPoint, LyonPoint, LyonPoint) {
    let mut pts = [a, b, c];
    pts.sort_by(|p, q| p.y.partial_cmp(&q.y).unwrap_or(std::cmp::Ordering::Equal));
    (pts[0], pts[1], pts[2])
}
