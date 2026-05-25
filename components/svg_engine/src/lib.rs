mod lengths;
mod paint;
mod path;
mod points;
mod render;
mod shapes;
mod transform;

pub use lengths::SvgLength;
pub use paint::{extract_fill_params, extract_geometry, extract_opacity, extract_stroke_params};
pub use render::render_svg_element;
pub use shapes::{
    FillParams, ParsedGeometry, SvgLineCap, SvgLineJoin, SvgRenderInput, SvgTag,
};
pub use transform::parse_transform;
