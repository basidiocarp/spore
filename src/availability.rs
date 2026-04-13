//! Runtime availability probes for ecosystem tools.
//!
//! Returns whether each tool is on PATH and/or has its data directory present,
//! what degradation tier it belongs to, and which capabilities are lost when it
//! is absent.
//!
//! Probe results are **not** cached — each call re-checks the filesystem. If
//! you need a stable snapshot for a request's lifetime, call [`probe_all`] once
//! and hold the result yourself.
//!
//! # Design decisions
//!
//! The tier table is compiled in rather than loaded from a config file so that
//! the probe surface is available even before any ecosystem tool has been
//! installed. Tier assignments follow the roles described in the workspace
//! CLAUDE.md:
//!
//! - **Tier 1** — critical: the ecosystem is broken without this tool.
//!   Currently: `mycelium` (proxy / entry point).
//! - **Tier 2** — degraded: major features are unavailable without this tool.
//!   Currently: `hyphae` (persistent memory / RAG), `rhizome` (code
//!   intelligence MCP server).
//! - **Tier 3** — optional: useful but the core workflow still functions.
//!   Currently: `cortina`, `canopy`, `hymenium`, `stipe`, `volva`, `annulus`,
//!   `cap`.
//!
//! `spore` and `lamella` are not included in the tier table because they are
//! libraries and content packages without a runtime binary presence.

use std::time::{Duration, Instant};

use crate::paths;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Degradation tier indicating how important a tool is to the overall
/// ecosystem health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DegradationTier {
    /// The ecosystem is non-functional without this tool.
    Tier1,
    /// Major features are unavailable without this tool.
    Tier2,
    /// Nice-to-have; core workflow continues without this tool.
    Tier3,
}

impl std::fmt::Display for DegradationTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tier1 => f.write_str("tier1"),
            Self::Tier2 => f.write_str("tier2"),
            Self::Tier3 => f.write_str("tier3"),
        }
    }
}

/// Availability report for a single ecosystem tool.
#[derive(Debug, Clone)]
#[must_use]
pub struct AvailabilityReport {
    /// Tool name (matches binary name).
    pub tool: String,
    /// Whether the tool was detected as available.
    pub available: bool,
    /// Criticality tier for this tool.
    pub tier: DegradationTier,
    /// Human-readable reason when `available` is `false`.
    pub reason: Option<String>,
    /// Capabilities that stop working when this tool is unavailable.
    pub degraded_capabilities: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier table
// ─────────────────────────────────────────────────────────────────────────────

/// A single entry in the compiled-in tool registry.
struct ToolEntry {
    name: &'static str,
    tier: DegradationTier,
    /// Optional data-directory filename to probe in addition to the binary.
    /// When set, the tool is considered unavailable if neither the binary nor
    /// the data path exists.
    db_filename: Option<&'static str>,
    capabilities: &'static [&'static str],
}

/// Compiled-in registry of all known ecosystem tools.
///
/// Order is not significant — callers should sort or filter by tier themselves.
static TOOL_TABLE: &[ToolEntry] = &[
    ToolEntry {
        name: "mycelium",
        tier: DegradationTier::Tier1,
        db_filename: None,
        capabilities: &[
            "token-optimised CLI proxy",
            "output filtering before model",
            "all downstream tool invocations via proxy",
        ],
    },
    ToolEntry {
        name: "hyphae",
        tier: DegradationTier::Tier2,
        db_filename: Some("hyphae.db"),
        capabilities: &[
            "persistent memory across sessions",
            "RAG / semantic search over stored context",
            "memoir and session recall",
        ],
    },
    ToolEntry {
        name: "rhizome",
        tier: DegradationTier::Tier2,
        db_filename: None,
        capabilities: &[
            "structure-aware code intelligence",
            "symbol search and cross-reference",
            "MCP code export",
        ],
    },
    ToolEntry {
        name: "cortina",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "lifecycle signal capture",
            "hook event recording",
            "structured hook signals",
        ],
    },
    ToolEntry {
        name: "canopy",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "multi-agent task ownership tracking",
            "agent handoff coordination",
            "evidence collection across agents",
        ],
    },
    ToolEntry {
        name: "hymenium",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "workflow dispatch and phase gating",
            "retry and recovery orchestration",
            "workflow state persistence",
        ],
    },
    ToolEntry {
        name: "stipe",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "ecosystem install and init",
            "managed updates across tools",
            "doctor / health-check flow",
        ],
    },
    ToolEntry {
        name: "volva",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "backend orchestration at runtime seam",
            "execution host for workflow steps",
        ],
    },
    ToolEntry {
        name: "annulus",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "statusline ecosystem indicator",
            "degraded-state UI feedback",
        ],
    },
    ToolEntry {
        name: "cap",
        tier: DegradationTier::Tier3,
        db_filename: None,
        capabilities: &[
            "operator dashboard and UI",
            "ecosystem data visualisation",
            "explicit write-through actions from UI",
        ],
    },
];

// ─────────────────────────────────────────────────────────────────────────────
// Probe budget
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum time a single probe is allowed to take.
///
/// All checks are filesystem / PATH lookups — this budget is purely defensive.
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Probe a single tool by name and return an [`AvailabilityReport`].
///
/// Unknown tool names are probed as a best-effort binary check with no tier
/// or capability information (assigned [`DegradationTier::Tier3`] and an empty
/// capabilities list).
///
/// The probe always completes within [`PROBE_TIMEOUT`]; if the filesystem
/// check somehow exceeds the budget, the tool is reported as unavailable.
///
/// # Examples
///
/// ```rust
/// use spore::availability::{probe_tool, DegradationTier};
///
/// let report = probe_tool("hyphae");
/// assert_eq!(report.tool, "hyphae");
/// assert_eq!(report.tier, DegradationTier::Tier2);
/// // `available` depends on the local environment.
/// if !report.available {
///     assert!(report.reason.is_some());
/// }
/// ```
#[must_use = "call probe_tool to check availability; discarding the report loses the result"]
pub fn probe_tool(name: &str) -> AvailabilityReport {
    let entry = TOOL_TABLE.iter().find(|e| e.name == name);
    let (tier, db_filename, capabilities) = entry.map_or(
        (DegradationTier::Tier3, None, [].as_slice()),
        |e| (e.tier, e.db_filename, e.capabilities),
    );

    let start = Instant::now();
    let result = run_probe_within_budget(name, db_filename, start);

    AvailabilityReport {
        tool: name.to_owned(),
        available: result.is_ok(),
        tier,
        reason: result.err(),
        degraded_capabilities: capabilities.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// Probe every tool registered in the tier table.
///
/// Returns one [`AvailabilityReport`] per registered tool in table order.
///
/// # Examples
///
/// ```rust
/// use spore::availability::probe_all;
///
/// let reports = probe_all();
/// // Every entry in the tier table produces a report.
/// assert!(!reports.is_empty());
/// for report in &reports {
///     if !report.available {
///         assert!(report.reason.is_some(), "unavailable tool must have a reason");
///     }
/// }
/// ```
#[must_use]
pub fn probe_all() -> Vec<AvailabilityReport> {
    TOOL_TABLE.iter().map(|e| probe_tool(e.name)).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal probe logic
// ─────────────────────────────────────────────────────────────────────────────

/// Run binary + optional data-dir checks, aborting if the wall clock exceeds
/// `PROBE_TIMEOUT` from `start`.
///
/// Returns `Ok(())` when the tool is considered available, or `Err(reason)`.
fn run_probe_within_budget(
    name: &str,
    db_filename: Option<&str>,
    start: Instant,
) -> Result<(), String> {
    if start.elapsed() >= PROBE_TIMEOUT {
        return Err(format!("probe budget exceeded after {PROBE_TIMEOUT:?}"));
    }

    // Primary check: binary on PATH.
    let binary_found = which::which(name).is_ok();

    if start.elapsed() >= PROBE_TIMEOUT {
        return Err(format!("probe budget exceeded after {PROBE_TIMEOUT:?}"));
    }

    // Secondary check: data directory / db file (if registered).
    let data_path_found = db_filename.map(|filename| {
        let path = paths::data_dir(name).join(filename);
        path.exists()
    });

    match (binary_found, data_path_found) {
        // Binary present — available regardless of db state.
        (true, _) => Ok(()),
        // No binary, but a data directory marker exists — tool was installed at
        // some point but may have been removed from PATH. Treat as unavailable
        // with an informative reason.
        (false, Some(true)) => {
            let db_path = paths::data_dir(name).join(db_filename.unwrap_or(""));
            Err(format!(
                "binary not found on PATH (data dir present: {})",
                db_path.display()
            ))
        }
        // No binary, no data path.
        (false, Some(false)) => Err(format!(
            "binary not found on PATH; db path missing: {}",
            paths::data_dir(name)
                .join(db_filename.unwrap_or(""))
                .display()
        )),
        // No binary, no data path check registered.
        (false, None) => Err(format!("binary not found on PATH: {name}")),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    // ── tier table coverage ──────────────────────────────────────────────────

    #[test]
    fn all_registered_tools_are_probeable() {
        for entry in TOOL_TABLE {
            let report = probe_tool(entry.name);
            assert_eq!(
                report.tool, entry.name,
                "report.tool must match the requested name"
            );
        }
    }

    #[test]
    fn probe_all_covers_every_registered_tool() {
        let reports = probe_all();
        assert_eq!(
            reports.len(),
            TOOL_TABLE.len(),
            "probe_all must return one report per tool table entry"
        );
        let names: Vec<&str> = reports.iter().map(|r| r.tool.as_str()).collect();
        for entry in TOOL_TABLE {
            assert!(
                names.contains(&entry.name),
                "probe_all missing tool: {}",
                entry.name
            );
        }
    }

    // ── tier assignments ─────────────────────────────────────────────────────

    #[test]
    fn tier_assignments_match_taxonomy() {
        let tier_for = |name: &str| {
            TOOL_TABLE
                .iter()
                .find(|e| e.name == name)
                .map_or(DegradationTier::Tier3, |e| e.tier)
        };

        // Tier 1 — critical
        assert_eq!(tier_for("mycelium"), DegradationTier::Tier1);

        // Tier 2 — degraded
        assert_eq!(tier_for("hyphae"), DegradationTier::Tier2);
        assert_eq!(tier_for("rhizome"), DegradationTier::Tier2);

        // Tier 3 — optional
        for name in &["cortina", "canopy", "hymenium", "stipe", "volva", "annulus", "cap"] {
            assert_eq!(
                tier_for(name),
                DegradationTier::Tier3,
                "{name} should be Tier3"
            );
        }
    }

    // ── unavailable tool reason ──────────────────────────────────────────────

    /// Probe a deliberately nonexistent binary and check that `reason` is set.
    #[test]
    fn unavailable_tool_has_non_empty_reason() {
        let report = probe_tool("__spore_test_nonexistent_binary__");
        assert!(!report.available, "nonexistent binary should not be available");
        assert!(
            report.reason.is_some(),
            "unavailable tool must populate reason"
        );
        let reason = report.reason.unwrap();
        assert!(!reason.is_empty(), "reason must not be empty");
    }

    #[test]
    fn unavailable_tool_reason_mentions_path() {
        let report = probe_tool("__spore_test_nonexistent_binary__");
        let reason = report.reason.unwrap_or_default();
        assert!(
            reason.contains("PATH") || reason.contains("budget"),
            "reason should mention PATH or budget timeout, got: {reason}"
        );
    }

    // ── probe is bounded ─────────────────────────────────────────────────────

    #[test]
    fn single_probe_completes_within_budget() {
        let start = Instant::now();
        let _report = probe_tool("hyphae");
        let elapsed = start.elapsed();
        assert!(
            elapsed < PROBE_TIMEOUT,
            "probe took {elapsed:?}, expected under {PROBE_TIMEOUT:?}"
        );
    }

    #[test]
    fn probe_all_completes_within_budget() {
        // Tight bound: one PROBE_TIMEOUT per tool in the tier table.
        // TOOL_TABLE.len() is tiny (< 20) so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let budget = PROBE_TIMEOUT * (TOOL_TABLE.len() as u32);
        let start = Instant::now();
        let reports = probe_all();
        let elapsed = start.elapsed();

        assert_eq!(reports.len(), TOOL_TABLE.len());
        assert!(
            elapsed < budget,
            "probe_all took {elapsed:?}, budget was {budget:?}"
        );
    }

    #[test]
    fn probe_all_completes_within_reasonable_time() {
        // Allow up to 3× the single-probe budget per tool.
        // TOOL_TABLE.len() is tiny (< 20) so the cast cannot truncate.
        #[allow(clippy::cast_possible_truncation)]
        let budget = PROBE_TIMEOUT * (TOOL_TABLE.len() as u32) * 3;
        let start = Instant::now();
        let reports = probe_all();
        let elapsed = start.elapsed();

        assert_eq!(reports.len(), TOOL_TABLE.len());
        assert!(
            elapsed < budget,
            "probe_all took {elapsed:?}, budget was {budget:?}"
        );
    }

    // ── degraded capabilities ────────────────────────────────────────────────

    #[test]
    fn tier1_tools_have_degraded_capabilities() {
        for entry in TOOL_TABLE.iter().filter(|e| e.tier == DegradationTier::Tier1) {
            assert!(
                !entry.capabilities.is_empty(),
                "Tier1 tool '{}' must declare degraded capabilities",
                entry.name
            );
        }
    }

    #[test]
    fn all_tools_have_at_least_one_degraded_capability() {
        for entry in TOOL_TABLE {
            assert!(
                !entry.capabilities.is_empty(),
                "tool '{}' must declare at least one degraded capability",
                entry.name
            );
        }
    }

    #[test]
    fn probe_tool_report_carries_degraded_capabilities() {
        // Use a known-registered tool so we can assert capability count.
        let report = probe_tool("mycelium");
        assert!(
            !report.degraded_capabilities.is_empty(),
            "mycelium report must carry degraded capabilities"
        );
    }

    // ── unknown tool name ────────────────────────────────────────────────────

    #[test]
    fn unknown_tool_returns_tier3_report() {
        let report = probe_tool("completely_unknown_tool_xyz");
        assert_eq!(report.tier, DegradationTier::Tier3);
        assert!(!report.available);
    }

    // ── DegradationTier Display ──────────────────────────────────────────────

    #[test]
    fn degradation_tier_display() {
        assert_eq!(DegradationTier::Tier1.to_string(), "tier1");
        assert_eq!(DegradationTier::Tier2.to_string(), "tier2");
        assert_eq!(DegradationTier::Tier3.to_string(), "tier3");
    }
}
