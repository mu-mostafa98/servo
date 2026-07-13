#[derive(Debug, Clone)]
pub struct RenderHints {
    pub vector_effect: Option<VectorEffect>,
    pub color_rendering: Option<ColorRendering>,
    pub color_interpolation: Option<ColorInterpolation>,
    pub shape_rendering: Option<ShapeRendering>,
    pub text_rendering: Option<TextRendering>,
    pub image_rendering: Option<ImageRendering>,
    pub paint_order: Option<PaintOrder>,
}

#[derive(Debug, Clone, Copy)]
pub enum VectorEffect { None, NonScalingStroke }

#[derive(Debug, Clone, Copy)]
pub enum ColorRendering { Auto, OptimizeSpeed, OptimizeQuality }

#[derive(Debug, Clone, Copy)]
pub enum ColorInterpolation { Auto, Srgb, LinearRGB }

#[derive(Debug, Clone, Copy)]
pub enum ShapeRendering { Auto, OptimizeSpeed, CrispEdges, GeometricPrecision }

#[derive(Debug, Clone, Copy)]
pub enum TextRendering { Auto, OptimizeSpeed, OptimizeLegibility, GeometricPrecision }

#[derive(Debug, Clone, Copy)]
pub enum ImageRendering { Auto, OptimizeSpeed, OptimizeQuality }

#[derive(Debug, Clone, Copy)]
pub enum PaintOrder { Normal }
