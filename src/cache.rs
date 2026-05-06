//! Thread-local cache namespace isolation.
//!
//! Provides scoped cache key prefixing without modifying existing global caches.
//! Use `with_cache_namespace` to run a closure with a namespace active.
//! All calls to `get_scoped_cache_key` inside the closure return prefixed keys.
//! Outside a namespace, keys are returned unchanged.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::BuildHasher;

thread_local! {
    static CACHE_NAMESPACE: RefCell<Option<String>> = const { RefCell::new(None) };
}

struct NamespaceGuard(Option<String>);

impl Drop for NamespaceGuard {
    fn drop(&mut self) {
        CACHE_NAMESPACE.with(|cell| *cell.borrow_mut() = self.0.take());
    }
}

/// Run `f` with `namespace` as the active cache namespace for this thread.
///
/// Cache keys constructed inside the closure via `get_scoped_cache_key` are
/// automatically prefixed with `{namespace}:`. Namespaces nest: the innermost
/// namespace wins and the outer is restored on exit, even if `f` panics.
pub fn with_cache_namespace<T>(namespace: &str, f: impl FnOnce() -> T) -> T {
    let prev = CACHE_NAMESPACE.with(|cell| cell.borrow().clone());
    CACHE_NAMESPACE.with(|cell| *cell.borrow_mut() = Some(namespace.to_string()));
    let _guard = NamespaceGuard(prev);
    f()
}

/// Return `key` prefixed with the active namespace when one is set.
///
/// - No namespace: `"foo"` → `"foo"`
/// - Namespace `"session-A"`: `"foo"` → `"session-A:foo"`
#[must_use]
pub fn get_scoped_cache_key(key: &str) -> String {
    CACHE_NAMESPACE.with(|cell| match cell.borrow().as_deref() {
        Some(ns) => format!("{ns}:{key}"),
        None => key.to_string(),
    })
}

/// Remove all entries from `cache` whose keys start with `"{namespace}:"`.
///
/// Use this to invalidate all cache entries that were inserted under a given
/// namespace. No-op when no matching entries exist.
pub fn clear_namespace_cache<V, S: BuildHasher>(
    namespace: &str,
    cache: &mut HashMap<String, V, S>,
) {
    let prefix = format!("{namespace}:");
    cache.retain(|k, _| !k.starts_with(&prefix));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_namespace_returns_key_unchanged() {
        let key = get_scoped_cache_key("my-key");
        assert_eq!(key, "my-key");
    }

    #[test]
    fn namespace_prefix_applied_inside_closure() {
        with_cache_namespace("session-A", || {
            let key = get_scoped_cache_key("my-key");
            assert_eq!(key, "session-A:my-key");
        });
    }

    #[test]
    fn namespace_restored_after_closure() {
        with_cache_namespace("session-A", || {});
        let key = get_scoped_cache_key("my-key");
        assert_eq!(key, "my-key");
    }

    #[test]
    fn nested_namespaces_inner_wins() {
        with_cache_namespace("outer", || {
            with_cache_namespace("inner", || {
                let key = get_scoped_cache_key("k");
                assert_eq!(key, "inner:k");
            });
            let key = get_scoped_cache_key("k");
            assert_eq!(key, "outer:k");
        });
    }

    #[test]
    fn clear_namespace_removes_prefixed_keys() {
        let mut cache: HashMap<String, &str> = HashMap::new();
        cache.insert("session-A:foo".into(), "v1");
        cache.insert("session-B:foo".into(), "v2");
        cache.insert("global-key".into(), "v3");
        clear_namespace_cache("session-A", &mut cache);
        assert!(!cache.contains_key("session-A:foo"));
        assert!(cache.contains_key("session-B:foo"));
        assert!(cache.contains_key("global-key"));
    }

    #[test]
    fn empty_namespace_produces_colon_prefix() {
        // Empty namespace is accepted and produces ":key" — callers should avoid it.
        with_cache_namespace("", || {
            let key = get_scoped_cache_key("foo");
            assert_eq!(key, ":foo");
        });
    }

    #[test]
    fn key_containing_colon_is_scoped_correctly() {
        with_cache_namespace("ns", || {
            let key = get_scoped_cache_key("a:b");
            assert_eq!(key, "ns:a:b");
        });
        // clear_namespace_cache("ns") removes it correctly because it starts with "ns:"
        let mut cache: HashMap<String, i32> = HashMap::new();
        cache.insert("ns:a:b".into(), 1);
        clear_namespace_cache("ns", &mut cache);
        assert!(cache.is_empty());
    }

    #[test]
    fn clear_namespace_does_not_match_longer_prefix() {
        // "ns:" does not match entries under "ns-extended:"
        let mut cache: HashMap<String, i32> = HashMap::new();
        cache.insert("ns:foo".into(), 1);
        cache.insert("ns-extended:foo".into(), 2);
        clear_namespace_cache("ns", &mut cache);
        assert!(!cache.contains_key("ns:foo"));
        assert!(cache.contains_key("ns-extended:foo"));
    }
}
