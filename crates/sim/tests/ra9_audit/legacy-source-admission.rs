//! UNCOMPILED/UNRUN bridge to three existing exact guard implementations. No bulk approval.
//! Legacy361/543 use independent declare/admit/finish plans in source-plans.
use super::*;
use source_routes::Route;
pub(super) fn require(
    a: &Audit,
    i: &Inputs,
    d: &Definitions,
    row: &Unused,
    route: Route,
    s: &scope_inputs::ObservedScope,
) -> AuditResult<()> {
    let accepted = s.accepted_source(row)?;
    match route {
        Route::Static116 => source_disposition::require(
            a,
            i,
            d,
            row,
            Some(&source_disposition::Approval {
                decision_id: accepted.recorded_decision,
                scoped_evidence_sha256: s.static_evidence_sha256(),
            }),
        ),
        Route::Metadata147 => metadata_admission::require(
            a,
            i,
            d,
            row,
            Some(&metadata_admission::Approval {
                recorded_decision: accepted.recorded_decision,
                source_census_sha256: s.model_sha256(),
            }),
        ),
        Route::Residual117 => residual_admission::require(
            a,
            i,
            d,
            row,
            Some(&residual_admission::Approval {
                recorded_decision: accepted.recorded_decision,
                live_census_sha256: s.model_sha256(),
                original_section_closure_record: s.original_closure_sha256(),
                required_native_receipts_record: accepted.native_receipts,
            }),
        ),
        _ => Err("foreign legacy source route".into()),
    }
}
