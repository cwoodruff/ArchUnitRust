use fixture_macros::transactional;

pub struct OrderService;

impl OrderService {
    #[transactional]
    pub fn place_order(&self) -> u32 {
        1
    }

    pub fn place_two_orders(&self) -> u32 {
        // Violation: bypasses the proxy of `place_order`.
        self.place_order() + self.place_order()
    }

    pub fn helper(&self) -> u32 {
        2
    }

    pub fn uses_helper(&self) -> u32 {
        self.helper()
    }
}

pub struct Client;

impl Client {
    pub fn place(&self, service: &OrderService) -> u32 {
        service.place_order()
    }
}
