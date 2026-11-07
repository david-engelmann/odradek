//! Hunter agents stalk a compute cluster for zombie workloads, idle services, forgotten namespaces, and load balancers serving nobody. Nothing is deleted on suspicion; every target is proven dead first.
//!
//! Odradek proves each kill before it fires and starts
//! from a reversible quarantine. It works only from recorded
//! or synthetic state and never connects to a live cluster.

pub mod bignum;
pub mod errors;
pub mod judgment;
pub mod observe;
pub mod paths;
pub mod plan;
pub mod pyjson;
pub mod pytext;
pub mod scan;
pub mod snapshot;

mod printable_table;

pub use errors::{Error, Result};
pub use judgment::{
    CONFIDENCE_THRESHOLD, FixturePlayer, JUDGMENT_POINT, JudgmentSource, check_answer,
    kill_call_request, trusted, validate_fixture,
};
pub use observe::build_observation;
pub use plan::{build_estate_plan, build_plan};
pub use scan::{classify, classify_entry, format_scan};
pub use snapshot::{Entry, Snapshot, parse_snapshot};
