pub struct Parser;

impl Parser {
    pub fn parse_unwrapped(&self, text: &str) -> u32 {
        text.parse::<u32>().unwrap()
    }

    pub fn parse_expected(&self, text: &str) -> u32 {
        text.parse::<u32>().expect("a number")
    }

    pub fn first_or_zero(&self, values: &[u32]) -> u32 {
        values.first().copied().unwrap_or(0)
    }

    pub fn handled(&self, text: &str) -> Option<u32> {
        text.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;

    #[test]
    fn unwrap_in_tests_is_fine() {
        let value: u32 = "1".parse().unwrap();
        assert_eq!(Parser.parse_unwrapped("1"), value);
    }
}
