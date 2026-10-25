//! Semantic Bridge metadata model for linking external metadata (Callers, Events, 0 GC Alloc).
//!
//! Provides `SemanticMetadataHeader`, `EventMetadata`, and `GcAllocReport`
//! for enriching AST slices with compact, LLM-friendly metadata comment blocks.

use serde::{Deserialize, Serialize};

/// Metadata regarding a C# or Unity event emitted by a method,
/// including its known subscribers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Name of the event, e.g. `"OnHealthChanged"`.
    pub name: String,
    /// List of subscriber files or component names, e.g. `["HealthBarView.cs"]`.
    pub subscribers: Vec<String>,
}

impl EventMetadata {
    /// Creates a new `EventMetadata`.
    pub fn new(name: impl Into<String>, subscribers: Vec<String>) -> Self {
        Self {
            name: name.into(),
            subscribers,
        }
    }

    /// Formats this event for inclusion in metadata header comments.
    ///
    /// Examples:
    /// - `OnHealthChanged (Subscribers: HealthBarView.cs)`
    /// - `OnHealthChanged` (if no subscribers are known)
    pub fn format_comment(&self) -> String {
        if self.subscribers.is_empty() {
            self.name.clone()
        } else {
            format!(
                "{} (Subscribers: {})",
                self.name,
                self.subscribers.join(", ")
            )
        }
    }
}

/// Garbage collection allocation analysis report for a method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcAllocReport {
    /// True if zero GC allocations were detected in the method.
    pub is_zero_alloc: bool,
    /// List of detected potential allocation points (e.g. `'new HeavyClass()'`, `LINQ '.Where()'`).
    pub potential_allocs: Vec<String>,
}

impl GcAllocReport {
    /// Creates a report indicating 0 GC allocations were detected.
    pub fn zero_alloc() -> Self {
        Self {
            is_zero_alloc: true,
            potential_allocs: Vec::new(),
        }
    }

    /// Creates a report with potential GC allocations.
    pub fn with_allocs(allocs: Vec<String>) -> Self {
        let is_zero = allocs.is_empty();
        Self {
            is_zero_alloc: is_zero,
            potential_allocs: allocs,
        }
    }

    /// Returns `true` if zero GC allocations were detected.
    pub fn is_zero(&self) -> bool {
        self.is_zero_alloc && self.potential_allocs.is_empty()
    }

    /// Returns the human-readable summary string.
    ///
    /// Examples:
    /// - `"0 GC alloc detected in this method"`
    /// - `"Potential GC allocs: 'new HeavyClass()', LINQ '.Where()'"`
    pub fn to_display_string(&self) -> String {
        if self.is_zero() {
            "0 GC alloc detected in this method".to_string()
        } else {
            format!("Potential GC allocs: {}", self.potential_allocs.join(", "))
        }
    }
}

impl std::fmt::Display for GcAllocReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display_string())
    }
}

/// Compact metadata header prepended to method slices for LLM context.
///
/// Encapsulates:
/// - Callers (up to 5 compact references: `File.cs:Line`)
/// - Events emitted by the method and their subscribers
/// - GC allocation status (0 GC alloc or list of potential allocations)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMetadataHeader {
    /// Callers (up to 5 compact references, e.g. `CombatSystem.cs:42`).
    pub callers: Vec<String>,
    /// Events emitted by this method or symbol.
    pub emitted_events: Vec<EventMetadata>,
    /// GC allocation status report.
    pub gc_alloc_status: GcAllocReport,
}

impl SemanticMetadataHeader {
    /// Creates a new `SemanticMetadataHeader`.
    pub fn new(
        callers: Vec<String>,
        emitted_events: Vec<EventMetadata>,
        gc_alloc_status: GcAllocReport,
    ) -> Self {
        Self {
            callers,
            emitted_events,
            gc_alloc_status,
        }
    }

    /// Builder method to add a caller reference (up to 5 max).
    #[must_use]
    pub fn with_caller(mut self, caller: impl Into<String>) -> Self {
        if self.callers.len() < 5 {
            self.callers.push(caller.into());
        }
        self
    }

    /// Builder method to add an emitted event.
    #[must_use]
    pub fn with_event(mut self, event: EventMetadata) -> Self {
        self.emitted_events.push(event);
        self
    }

    /// Formats the metadata as a compact C# comment header (up to 5 lines).
    ///
    /// Example output:
    /// ```csharp
    /// // [META] Callers: CombatSystem.cs:42, Projectile.cs:88
    /// // [META] Events emitted: OnHealthChanged (Subscribers: HealthBarView.cs)
    /// // [META] Allocations: 0 GC alloc detected in this method
    /// ```
    pub fn to_header_comments(&self) -> String {
        let mut lines = Vec::with_capacity(3);

        if !self.callers.is_empty() {
            let callers_str = self
                .callers
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("// [META] Callers: {callers_str}"));
        }

        if !self.emitted_events.is_empty() {
            let events_str = self
                .emitted_events
                .iter()
                .map(|e| e.format_comment())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("// [META] Events emitted: {events_str}"));
        }

        lines.push(format!(
            "// [META] Allocations: {}",
            self.gc_alloc_status.to_display_string()
        ));

        lines.join("\n")
    }

    /// Prepends the header comments to the target method source code.
    pub fn prepend_to(&self, code: &str) -> String {
        let header = self.to_header_comments();
        if header.is_empty() {
            code.to_string()
        } else {
            format!("{header}\n{code}")
        }
    }
}
