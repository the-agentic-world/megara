#[derive(Debug)]
pub(crate) struct PersistedSpec {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) persisted_at: String,
    pub(crate) payload: String,
}

#[derive(Debug)]
pub(crate) struct PersistedPlan {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) persisted_at: String,
    pub(crate) payload: String,
}
