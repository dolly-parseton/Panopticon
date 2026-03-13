pub struct IterSource {
    pub(crate) reference: String,
}

impl IterSource {
    pub fn array(reference: impl Into<String>) -> Self {
        IterSource {
            reference: reference.into(),
        }
    }
    pub fn map(reference: impl Into<String>) -> Self {
        IterSource {
            reference: reference.into(),
        }
    }
}

pub struct GuardSource {
    pub(crate) reference: String,
}

impl GuardSource {
    pub fn boolean(reference: impl Into<String>) -> Self {
        GuardSource {
            reference: reference.into(),
        }
    }
}
