//! UNCOMPILED/UNRUN actual final baseline first, no permanently unaccepted table assertion.
use super::*;
#[test]
fn ra9_legacy_authority_missing_end_rejects_instead_of_widening_to_eof() -> AuditResult<()> {
    audit_at(&super::super::data_dir(), &super::super::reference_dir())?;
    let r = legacy904_support::records(
        include_str!("legacy904-document-guards.tsv"),
        "authority_id\tdocument_path\tsection_start\tsection_end\tspan_bytes\tspan_fnv1a64\tsampled_span_sha256",
    )?;
    let row = r.iter().find(|x| x[0] == "R34").ok_or("actual authority")?;
    let t = std::fs::read_to_string(legacy904_support::root().join(row[1]))
        .map_err(|e| e.to_string())?;
    legacy904_support::document_span(&t, row[2], row[3], row[4], row[5])?;
    let at = t.find(row[2]).ok_or("start")?;
    let end = at + row[2].len() + t[at + row[2].len()..].find(row[3]).ok_or("real end")?;
    let bad = &t[..end];
    let e = legacy904_support::document_span(bad, row[2], row[3], row[4], row[5])
        .expect_err("real missing end");
    assert!(e.contains("missing authority END"), "{e}");
    Ok(())
}
