use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct DomainError;

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("domain error")
    }
}

impl Error for DomainError {}

pub struct Service;

impl Service {
    pub fn boxed(&self) -> Result<(), Box<dyn Error>> {
        Err(Box::new(DomainError))
    }

    pub fn boxed_send(&self) -> Result<u32, Box<dyn Error + Send + Sync + 'static>> {
        Ok(1)
    }

    pub fn stringly(&self) -> Result<(), String> {
        Err(String::from("failed"))
    }

    pub fn static_str(&self) -> Result<(), &'static str> {
        Err("failed")
    }

    pub fn unit(&self) -> Result<(), ()> {
        Err(())
    }

    pub fn typed(&self) -> Result<(), DomainError> {
        Err(DomainError)
    }

    pub fn infallible(&self) -> u32 {
        7
    }
}

pub fn free_boxed() -> Result<(), Box<dyn Error>> {
    Ok(())
}
