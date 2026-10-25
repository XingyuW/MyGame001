//! Whole-audit acceptance controls. Every replacement is local to one synchronous audit call.
//! The real loaders, adapters, source admissions and all final reconciliations remain in use.
use super::*;
use policy_admission::SourceAcceptance;
use source_routes::Route;

fn nonfirst_key(route: &str, file: &str, section: &str, key: &str) -> AuditResult<SourceKey> {
    // Select identity from the actual disposition route assignment, independently of acceptance.
    let text =
        std::fs::read_to_string(super::super::reference_dir().join("ra9_source_dispositions.tsv"))
            .map_err(|e| e.to_string())?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or("actual disposition header absent")?
        .split('\t')
        .collect::<Vec<_>>();
    let at = |name: &str| {
        header
            .iter()
            .position(|v| *v == name)
            .ok_or_else(|| format!("actual disposition column missing: {name}"))
    };
    let (fi, si, ki, ri) = (at("file")?, at("section")?, at("key")?, at("route")?);
    let mut actual = Vec::new();
    for line in lines {
        let row = line.split('\t').collect::<Vec<_>>();
        if row.len() != header.len() {
            return Err("actual disposition width changed".into());
        }
        if row[ri] == route {
            actual.push(SourceKey::new(row[fi], row[si], row[ki]));
        }
    }
    let selected = SourceKey::new(file, section, key);
    if actual.get(1) != Some(&selected) || actual.first() == Some(&selected) {
        return Err(format!("required nonfirst {route} source identity changed"));
    }
    Ok(selected)
}

fn whole_control(
    route: Route,
    name: &str,
    file: &str,
    section: &str,
    key: &str,
) -> AuditResult<()> {
    let data = super::super::data_dir();
    let references = super::super::reference_dir();
    // This accepted baseline must finish the actual complete audit before any negative can count.
    super::audit_at(&data, &references)?;
    let selected = nonfirst_key(name, file, section, key)?;
    let accepted = policy_admission::source_acceptance(&selected)?
        .ok_or("nonfirst installed source acceptance missing")?;
    let scope = scope_inputs::ObservedScope::read(&references)?;
    // Expected bindings are independently declared from Route, key and observed producer scope.
    // No expected identity or route is copied from the acceptance being tested.
    let (consumer, guard, receipt_route) = match route {
        Route::Static116 | Route::Metadata147 | Route::Residual117 => (
            format!(
                "current-model-policy={route:?};source={file}:{section}:{key};model-census={};original-closure={}",
                scope.model_sha256(),
                scope.original_closure_sha256()
            ),
            format!("ra9.source-policy.{route:?}.{section}.{key}"),
            match route {
                Route::Static116 => "static/exact-source-current-model/static116",
                Route::Metadata147 => "static/exact-source-current-model/metadata147",
                Route::Residual117 => "static/exact-source-current-model/residual117",
                _ => return Err("foreign legacy control route".into()),
            },
        ),
        Route::Masked19 => (
            "weapon.gun_105mm_e@rules:105mmE".to_owned(),
            "ra9.arcing-range-gravity.105mmE.v1".to_owned(),
            "static/exact-source-current-model/masked19",
        ),
        _ => return Err("foreign single-source control route".into()),
    };
    assert_eq!(accepted.consumer_bindings, consumer);
    assert_eq!(accepted.model_guard_id, guard);
    let (installed_route, hash) = accepted
        .native_receipts
        .split_once('@')
        .ok_or("valid installed receipt has no artifact hash")?;
    assert_eq!(installed_route, receipt_route);
    assert!(hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()));
    let foreign = format!("static/exact-source-current-model/foreign-control@{hash}");
    let hashless = format!("{receipt_route}@");
    let duplicate = format!("{};{}", accepted.native_receipts, accepted.native_receipts);
    let missing_error = if route == Route::Masked19 {
        "individual source model decision/final scope/native receipts missing"
    } else {
        "individual exact source policy/consumer/final scope/real receipts missing"
    };
    let identity_error = "source acceptance does not bind exact current consumer/model/reason";
    let cases = [
        (
            "foreign-consumer",
            SourceAcceptance {
                consumer_bindings: "foreign-current-consumer",
                ..accepted.clone()
            },
            identity_error,
        ),
        (
            "foreign-guard",
            SourceAcceptance {
                model_guard_id: "foreign-current-model-guard",
                ..accepted.clone()
            },
            identity_error,
        ),
        (
            "missing-route",
            SourceAcceptance {
                native_receipts: "",
                ..accepted.clone()
            },
            missing_error,
        ),
        (
            "foreign-route",
            SourceAcceptance {
                native_receipts: &foreign,
                ..accepted.clone()
            },
            "missing/foreign exact source evidence routes",
        ),
        (
            "malformed-receipt",
            SourceAcceptance {
                native_receipts: receipt_route,
                ..accepted.clone()
            },
            "scoped kind/route@actual-artifact-sha256 required",
        ),
        (
            "hashless-receipt",
            SourceAcceptance {
                native_receipts: &hashless,
                ..accepted.clone()
            },
            "malformed/duplicate real evidence route",
        ),
        (
            "duplicate-route",
            SourceAcceptance {
                native_receipts: &duplicate,
                ..accepted.clone()
            },
            "malformed/duplicate real evidence route",
        ),
    ];
    let context = format!("source admission {route:?} {file}:{section}:{key}: ");
    let mut failures = Vec::new();
    for (variant, replacement, expected) in cases {
        match super::audit_at_with_source_acceptance(&data, &references, Some(&replacement)) {
            Ok(()) => {
                eprintln!(
                    "RA9_SINGLE_SOURCE_CONTROL {name} {variant} ESCAPED_FINAL_RECONCILIATION"
                );
                failures.push(format!("{variant}: accepted through final reconciliation"));
            }
            Err(error) if error.starts_with(&context) && error.contains(expected) => {
                eprintln!("RA9_SINGLE_SOURCE_CONTROL {name} {variant} REJECTED {error}");
            }
            Err(error) => {
                eprintln!("RA9_SINGLE_SOURCE_CONTROL {name} {variant} WRONG_FAILURE {error}");
                failures.push(format!(
                    "{variant}: wrong row, setup or predicate failure: {error}"
                ));
            }
        }
    }
    // A later full positive verifies that replacements did not leak into another invocation.
    super::audit_at(&data, &references)?;
    assert!(failures.is_empty(), "{name}: {}", failures.join("\n"));
    Ok(())
}

#[test]
fn ra9_single_source_static116_nonfirst_acceptance_controls() -> AuditResult<()> {
    whole_control(Route::Static116, "static116", "art", "CABHUT", "Height")
}

#[test]
fn ra9_single_source_metadata147_nonfirst_acceptance_controls() -> AuditResult<()> {
    whole_control(
        Route::Metadata147,
        "metadata147",
        "rules",
        "AEGIS",
        "Points",
    )
}

#[test]
fn ra9_single_source_residual117_nonfirst_acceptance_controls() -> AuditResult<()> {
    whole_control(
        Route::Residual117,
        "residual117",
        "rules",
        "AAHeatSeeker2",
        "ROT",
    )
}

#[test]
fn ra9_single_source_masked19_nonfirst_acceptance_controls() -> AuditResult<()> {
    whole_control(Route::Masked19, "masked19", "rules", "105mmE", "Speed")
}
