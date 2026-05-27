use euclid::Transform2D;
use kurbo::{BezPath, Point as KurboPoint};
use malloc_size_of_derive::MallocSizeOf;
use webrender_api::ColorF;

use crate::lengths::SvgLength;

/// SVG element tag enum — identifies the type of SVG element being rendered.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgTag {
    Rect,
    Circle,
    Ellipse,
    Line,
    Polyline,
    Polygon,
    Path,
    Text,
    Image,
    Use,
    LinearGradient,
    RadialGradient,
    ClipPath,
    Mask,
    Svg,
    G,
    Defs,
    Unknown,
}

impl SvgTag {
    pub fn from_str(s: &str) -> Self {
        match s {
            "rect" => SvgTag::Rect,
            "circle" => SvgTag::Circle,
            "ellipse" => SvgTag::Ellipse,
            "line" => SvgTag::Line,
            "polyline" => SvgTag::Polyline,
            "polygon" => SvgTag::Polygon,
            "path" => SvgTag::Path,
            "text" => SvgTag::Text,
            "image" => SvgTag::Image,
            "use" => SvgTag::Use,
            "linearGradient" => SvgTag::LinearGradient,
            "radialGradient" => SvgTag::RadialGradient,
            "clipPath" => SvgTag::ClipPath,
            "mask" => SvgTag::Mask,
            "svg" => SvgTag::Svg,
            "g" => SvgTag::G,
            "defs" => SvgTag::Defs,
            _ => SvgTag::Unknown,
        }
    }

    pub fn is_basic_shape(&self) -> bool {
        matches!(self, SvgTag::Rect | SvgTag::Circle | SvgTag::Ellipse | SvgTag::Line | SvgTag::Polyline | SvgTag::Polygon | SvgTag::Path)
    }

    pub fn is_container(&self) -> bool {
        matches!(self, SvgTag::Svg | SvgTag::G | SvgTag::Defs | SvgTag::Use)
    }
}

/// Parsed geometry for a basic SVG shape.
#[derive(Debug, Clone, MallocSizeOf)]
pub enum ParsedGeometry {
    Rect {
        x: Option<SvgLength>,
        y: Option<SvgLength>,
        width: Option<SvgLength>,
        height: Option<SvgLength>,
        rx: Option<SvgLength>,
        ry: Option<SvgLength>,
    },
    Circle {
        cx: Option<SvgLength>,
        cy: Option<SvgLength>,
        r: Option<SvgLength>,
    },
    Ellipse {
        cx: Option<SvgLength>,
        cy: Option<SvgLength>,
        rx: Option<SvgLength>,
        ry: Option<SvgLength>,
    },
    Line {
        x1: Option<SvgLength>,
        y1: Option<SvgLength>,
        x2: Option<SvgLength>,
        y2: Option<SvgLength>,
    },
    Polyline {
        #[ignore_malloc_size_of = "Vec<KurboPoint> does not implement MallocSizeOf"]
        points: Vec<KurboPoint>,
    },
    Polygon {
        #[ignore_malloc_size_of = "Vec<KurboPoint> does not implement MallocSizeOf"]
        points: Vec<KurboPoint>,
    },
    Path {
        #[ignore_malloc_size_of = "BezPath does not implement MallocSizeOf"]
        path: BezPath,
    },
    None,
}

/// SVG line cap style.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgLineCap {
    Butt,
    Round,
    Square,
}

/// SVG line join style.
#[derive(Debug, Clone, Copy, PartialEq, MallocSizeOf)]
pub enum SvgLineJoin {
    Miter,
    Round,
    Bevel,
}

/// Fill parameters extracted from computed style.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct FillParams {
    pub color: Option<ColorF>,
    pub opacity: f32,
}

/// Stroke parameters extracted from computed style.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct StrokeParams {
    pub color: Option<ColorF>,
    pub width: f32,
    pub opacity: f32,
    pub dasharray: Option<Vec<f32>>,
    pub dashoffset: f32,
    pub linecap: SvgLineCap,
    pub linejoin: SvgLineJoin,
    pub miterlimit: f32,
}

impl Default for StrokeParams {
    fn default() -> Self {
        Self {
            color: None,
            width: 1.0,
            opacity: 1.0,
            dasharray: None,
            dashoffset: 0.0,
            linecap: SvgLineCap::Butt,
            linejoin: SvgLineJoin::Miter,
            miterlimit: 4.0,
        }
    }
}

/// Full input data for the renderer — built once per element by the integration layer.
#[derive(Debug, Clone, MallocSizeOf)]
pub struct SvgRenderInput {
    pub tag: SvgTag,
    pub geometry: ParsedGeometry,
    pub fill: FillParams,
    pub stroke: StrokeParams,
    pub transform: Option<Transform2D<f32, (), ()>>,
    pub clip_path: Option<String>,
    pub opacity: f32,
}

impl SvgRenderInput {
    pub fn new(tag: SvgTag) -> Self {
        Self {
            tag,
            geometry: ParsedGeometry::None,
            fill: FillParams {
                color: Some(ColorF::BLACK),
                opacity: 1.0,
            },
            stroke: StrokeParams::default(),
            transform: None,
            clip_path: None,
            opacity: 1.0,
        }
    }
}
