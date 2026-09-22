pub mod lower;

pub struct Top {
    pub value: u32,
}

pub fn top_level() -> u32 {
    lower::Lower::default().0.value
}
