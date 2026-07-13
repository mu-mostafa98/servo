pub mod attr_parsers;
pub(crate) mod rectangle;

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
}
