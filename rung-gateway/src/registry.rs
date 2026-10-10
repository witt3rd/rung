//! Which instances the gateway fronts.

/// One instance as the gateway holds it. The key is the owner's key for that
/// host; it never leaves this struct except as the bearer of an upstream
/// request.
#[derive(Clone)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// `http://host:port`, no trailing slash.
    pub url: String,
    pub key: String,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

/// The source of instances, asked on every request so a source that reads a
/// folder sees instances come and go.
pub trait Registry: Send + Sync + 'static {
    fn list(&self) -> Vec<Instance>;

    fn get(&self, id: &str) -> Option<Instance> {
        self.list().into_iter().find(|i| i.id == id)
    }
}

/// A fixed list, from the config file.
pub struct StaticRegistry(pub Vec<Instance>);

impl Registry for StaticRegistry {
    fn list(&self) -> Vec<Instance> {
        self.0.clone()
    }
}
