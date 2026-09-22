use super::Top;

#[derive(Default)]
pub struct Lower(pub Top);

impl Default for Top {
    fn default() -> Self {
        Top { value: 0 }
    }
}

impl Lower {
    pub fn uses_parent(&self) -> u32 {
        crate::upper::top_level()
    }
}
