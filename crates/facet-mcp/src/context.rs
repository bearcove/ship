use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Type-map context passed to every tool handler.
///
/// Tools retrieve their dependencies by type:
/// ```ignore
/// let http = ctx.get::<reqwest::Client>();
/// ```
pub struct ToolCtx {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ToolCtx {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a value by its type. Overwrites any previous value of the same type.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Get a reference to a value by its type. Panics if not found.
    pub fn get<T: Send + Sync + 'static>(&self) -> &T {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "ToolCtx: missing dependency {}",
                    std::any::type_name::<T>()
                )
            })
    }

    /// Try to get a reference to a value by its type. Returns None if not found.
    pub fn try_get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut ctx = ToolCtx::new();
        ctx.insert(42u32);
        ctx.insert("hello".to_owned());
        assert_eq!(*ctx.get::<u32>(), 42);
        assert_eq!(ctx.get::<String>(), "hello");
    }

    #[test]
    fn try_get_missing() {
        let ctx = ToolCtx::new();
        assert!(ctx.try_get::<u32>().is_none());
    }

    #[test]
    #[should_panic(expected = "missing dependency")]
    fn get_missing_panics() {
        let ctx = ToolCtx::new();
        let _ = ctx.get::<u32>();
    }

    #[test]
    fn overwrite() {
        let mut ctx = ToolCtx::new();
        ctx.insert(1u32);
        ctx.insert(2u32);
        assert_eq!(*ctx.get::<u32>(), 2);
    }
}
