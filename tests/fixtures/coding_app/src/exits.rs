pub struct Terminator;

impl Terminator {
    pub fn stop(&self) -> ! {
        std::process::exit(1)
    }

    pub fn abort_now(&self) -> ! {
        std::process::abort()
    }

    pub fn keep_going(&self) -> u32 {
        0
    }
}
