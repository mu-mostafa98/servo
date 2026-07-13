pub mod attr_parsers;
pub(crate) mod circle;
pub(crate) mod ellipse;
pub(crate) mod line;
pub(crate) mod polygon;
pub(crate) mod polyline;
pub(crate) mod rectangle;

pub use self::circle::Circle;
pub use self::ellipse::Ellipse;
pub use self::line::Line;
pub use self::polygon::Polygon;
pub use self::polyline::Polyline;
pub use self::rectangle::Rectangle;

pub trait AttrAccessor {
    fn get_attr(&self, name: &str) -> Option<String>;
}

impl<F> AttrAccessor for F
where
    F: Fn(&str) -> Option<String>,
{
    fn get_attr(&self, name: &str) -> Option<String> {
        (self)(name)
    }
}

pub trait BuildFromElement: Sized {
    fn from_attrs(font_size: f32, attrs: &impl AttrAccessor) -> Option<Self>;
}

#[derive(Debug, Clone)]
pub enum Shape {
    Rect(Rectangle),
    Circle(Circle),
    Ellipse(Ellipse),
    Line(Line),
    Polygon(Polygon),
    Polyline(Polyline),
}
