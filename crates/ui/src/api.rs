//! Where the client sends api/ws requests. The SSR shell injects the
//! origin as an `api-base` meta; an empty value means same origin, which
//! is the single-origin default.

use leptos::prelude::document;

/// The meta's origin, or `""` when absent (same origin).
pub fn base() -> String {
    document()
        .query_selector("meta[name='api-base']")
        .ok()
        .flatten()
        .and_then(|meta| meta.get_attribute("content"))
        .unwrap_or_default()
}
