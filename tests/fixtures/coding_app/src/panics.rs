pub struct Guard;

impl Guard {
    pub fn explicit(&self, value: u32) {
        if value == 0 {
            panic!("value must not be zero");
        }
    }

    pub fn not_yet(&self) -> u32 {
        todo!()
    }

    pub fn never(&self) -> u32 {
        unimplemented!("later")
    }

    pub fn exhaustive(&self, value: u32) -> u32 {
        match value {
            0 => 1,
            _ => unreachable!("only zero is passed"),
        }
    }

    pub fn calm(&self) -> u32 {
        1
    }
}
