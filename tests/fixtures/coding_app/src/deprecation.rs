#[deprecated(note = "use NewApi")]
pub struct OldApi;

#[allow(deprecated)]
impl OldApi {
    pub fn run(&self) -> u32 {
        1
    }
}

pub struct NewApi;

impl NewApi {
    #[deprecated = "use run"]
    pub fn legacy_run(&self) -> u32 {
        0
    }

    pub fn run(&self) -> u32 {
        2
    }
}

#[deprecated]
pub fn old_helper() -> u32 {
    3
}

pub struct Caller;

#[allow(deprecated)]
impl Caller {
    pub fn uses_deprecated_type(&self) -> u32 {
        OldApi.run()
    }

    pub fn uses_deprecated_method(&self) -> u32 {
        NewApi.legacy_run()
    }

    pub fn uses_deprecated_function(&self) -> u32 {
        old_helper()
    }

    pub fn uses_current_api(&self) -> u32 {
        NewApi.run()
    }
}
