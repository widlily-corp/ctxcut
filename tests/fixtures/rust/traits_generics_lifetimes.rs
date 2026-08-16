//! Generic functions with trait bounds, where clauses, and complex lifetimes.

use std::collections::HashMap;
use std::fmt::Debug;

/// Trait defining a bidirectional data transformer contract.
pub trait Transformable {
    type Output;
    fn transform(&self) -> Self::Output;
    fn validate(&self) -> bool;
}

/// Trait for items that can be serialized into a byte buffer representation.
pub trait ByteSerializable: Send + Sync + 'static {
    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, String>
    where
        Self: Sized;
}

/// Generic pipeline filter processor with where clause constraints.
pub fn transform<T, R>(input: T) -> R
where
    T: Transformable<Output = R> + Clone + Debug,
    R: Default + Send + 'static,
{
    if !input.validate() {
        return R::default();
    }
    input.transform()
}

/// Multi-stage generic batch processor operating on lifetime-bound references.
pub fn process_batch<'a, 'b, T, K, V>(
    items: &'a [T],
    key_extractor: impl Fn(&'a T) -> K,
    val_transformer: impl Fn(&'a T) -> Result<V, &'b str>,
) -> Result<HashMap<K, V>, &'b str>
where
    K: std::hash::Hash + Eq + 'a,
    V: Clone + 'static,
{
    let mut map = HashMap::with_capacity(items.len());
    for item in items {
        let key = key_extractor(item);
        let val = val_transformer(item)?;
        map.insert(key, val);
    }
    Ok(map)
}

/// Wrapper struct illustrating trait bounds and generic struct definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineContainer<T, M>
where
    T: Clone + Debug,
    M: Default,
{
    pub payload: T,
    pub metadata: M,
    pub trace_id: String,
}

impl<T, M> PipelineContainer<T, M>
where
    T: Clone + Debug,
    M: Default,
{
    pub fn new(payload: T, trace_id: impl Into<String>) -> Self {
        Self {
            payload,
            metadata: M::default(),
            trace_id: trace_id.into(),
        }
    }

    pub fn map_payload<F, U>(self, f: F) -> PipelineContainer<U, M>
    where
        F: FnOnce(T) -> U,
        U: Clone + Debug,
    {
        PipelineContainer {
            payload: f(self.payload),
            metadata: self.metadata,
            trace_id: self.trace_id,
        }
    }
}
