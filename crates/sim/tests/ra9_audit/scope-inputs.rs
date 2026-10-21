//! UNCOMPILED/UNRUN reads actual metadata outputs of --ra9-scope; expected tables are not
//! observations. Root separately authenticates producer inputs, artifact SHA and decisions.
//! Fresh actual file-set/byte-count/FNV checks detect ordinary source drift without new deps.
//! FNV is not cryptographic authentication; root's actual SHA256 verification remains required.
use super::*;
pub(super) struct ObservedScope {
    original: String,
    model_surface: String,
    special: String,
    model_sha: String,
    original_sha: String,
    original_closure_sha: String,
    static_sha: String,
    binding: String,
}
fn good_hash(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn sources(dir: &Path, out: &mut BTreeSet<PathBuf>) -> AuditResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        let t = e.file_type().map_err(|e| e.to_string())?;
        if t.is_symlink() {
            return Err("scope source symlink needs explicit review".into());
        }
        if t.is_dir() {
            sources(&e.path(), out)?;
        } else if t.is_file() && e.path().extension().is_some_and(|s| s == "rs") {
            out.insert(e.path());
        }
    }
    Ok(())
}
pub(super) fn live_model(root: &Path, text: &str) -> AuditResult<()> {
    let mut actual = BTreeSet::new();
    for e in std::fs::read_dir(root.join("crates")).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let path = e.path().join("src");
        if path.is_dir() {
            sources(&path, &mut actual)?;
        }
    }
    let mut recorded = BTreeSet::new();
    for r in metadata(text, "path\tsha256\tfnv1a64\tbytes", 4)? {
        let relative = Path::new(r[0]);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("scope path escapes repository".into());
        }
        let path = root.join(relative);
        if !recorded.insert(path.clone()) || !good_hash(r[1]) {
            return Err("duplicate/malformed actual model census row".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut h = sim::Fnv1a64::new();
        h.write(&bytes);
        if format!("{:016x}", h.finish()) != r[2] || bytes.len().to_string() != r[3] {
            return Err(
                "actual source bytes changed since observed model census; regenerate/review/rebind"
                    .into(),
            );
        }
    }
    if actual != recorded {
        return Err(
            "actual whole Rust source file set differs from independently produced census".into(),
        );
    }
    Ok(())
}
fn metadata<'a>(text: &'a str, header: &str, width: usize) -> AuditResult<Vec<Vec<&'a str>>> {
    if text.lines().next() != Some(header) {
        return Err("actual scope metadata header".into());
    }
    text.lines()
        .skip(1)
        .map(|s| {
            let r = s.split('\t').collect::<Vec<_>>();
            if r.len() != width {
                Err("actual scope metadata width".into())
            } else {
                Ok(r)
            }
        })
        .collect()
}
impl ObservedScope {
    pub(super) fn read(references: &Path) -> AuditResult<Self> {
        let read = |n: &str| {
            std::fs::read_to_string(references.join(n))
                .map_err(|e| format!("real scope producer output {n}: {e}"))
        };
        let original = read("ra9_scope75_original.tsv")?;
        let model_surface = read("ra9_model_surface.txt")?;
        let special = read("ra9_special_original.tsv")?;
        let model = read("ra9_model_census.tsv")?;
        let observed = read("ra9_scope_observation.tsv")?;
        let approvals = read("ra9_scope_approval.tsv")?;
        let o = metadata(
            &observed,
            "original_sha256\tmodel_census_sha256\tmodel_surface_sha256\toriginal_closure_sha256\tspecial_original_sha256",
            5,
        )?;
        let a = metadata(
            &approvals,
            "original_sha256\tmodel_census_sha256\tmodel_surface_sha256\toriginal_closure_sha256\tspecial_original_sha256\tstatic_evidence_sha256\trecorded_scope_review\tfinal_input_sha256",
            8,
        )?;
        if o.len() != 1
            || a.len() != 1
            || o[0] != a[0][..5]
            || a[0][6].is_empty()
            || a[0][6] == "UNACCEPTED"
            || !a[0][..6].iter().all(|v| good_hash(v))
            || a[0][7] != policy_admission::input_binding()
            || !good_hash(a[0][7])
        {
            return Err(
                "unverified/mismatched actual producer scope/root decision/final input binding"
                    .into(),
            );
        }
        if o[0][0] != "adc869ca037db5141f562f18092d022045e194de0d1204a46b9ec8372b2b826f" {
            return Err("original R2 source identity changed".into());
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        live_model(&root, &model)?;
        // The producer calculates the SHA256 fields from actual generated bytes; root must compare
        // all output bytes and hashes before signing approval. These strings are not self-authentication.
        Ok(Self {
            original,
            model_surface,
            special,
            model_sha: o[0][1].into(),
            original_sha: o[0][0].into(),
            original_closure_sha: o[0][3].into(),
            static_sha: a[0][5].into(),
            binding: a[0][7].into(),
        })
    }
    pub(super) fn guard75(&self) -> guard75_admission::Scope<'_> {
        guard75_admission::Scope {
            observed_original: &self.original,
            original_sha256: &self.original_sha,
            model_census_sha256: &self.model_sha,
            observed_model_scope: &self.model_surface,
            verified_final_binding: &self.binding,
        }
    }
    pub(super) fn special_original(&self) -> &str {
        &self.special
    }
    pub(super) fn model_sha256(&self) -> &str {
        &self.model_sha
    }
    pub(super) fn static_evidence_sha256(&self) -> &str {
        &self.static_sha
    }
    pub(super) fn original_closure_sha256(&self) -> &str {
        &self.original_closure_sha
    }
    pub(super) fn accepted_source(
        &self,
        row: &Unused,
    ) -> AuditResult<policy_admission::SourceAcceptance<'static>> {
        let a = policy_admission::source_acceptance(&row.atom.key)?
            .ok_or("missing individual exact source acceptance")?;
        if a.key != row.atom.key
            || a.exact != row.exact_value
            || a.reason != row.reason
            || a.recorded_decision.trim().is_empty()
            || a.recorded_decision == "UNACCEPTED"
            || a.final_input_sha256 != self.binding
            || a.consumer_bindings.is_empty()
            || a.model_guard_id.is_empty()
            || a.native_receipts.is_empty()
            || a.native_receipts == "UNRUN"
        {
            return Err(
                "individual exact source policy/consumer/final scope/real receipts missing".into(),
            );
        }
        Ok(a)
    }
    pub(super) fn require_routes(&self, text: &str, want: &BTreeSet<&str>) -> AuditResult<()> {
        let mut got = BTreeSet::new();
        for token in text.split(';') {
            let (id, h) = token
                .split_once('@')
                .ok_or("scoped kind/route@actual-artifact-sha256 required")?;
            if !matches!(
                id.split('/').next(),
                Some("native" | "static" | "client" | "loaded")
            ) || !good_hash(h)
                || !got.insert(id)
            {
                return Err("malformed/duplicate real evidence route".into());
            }
        }
        if &got != want {
            return Err("missing/foreign exact source evidence routes".into());
        }
        Ok(())
    }
    pub(super) fn return_fire_decision(&self) -> AuditResult<fire_audit::ReturnPolicyDecision> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/reference/ra9_return_fire_policy.tsv");
        let text = std::fs::read_to_string(root).map_err(|e| e.to_string())?;
        let r = metadata(
            &text,
            "acceptance\trecorded_decision\texplicit_assumption\tfinal_input_sha256",
            4,
        )?;
        if r.len() != 1
            || r[0][1].is_empty()
            || r[0][1] == "UNACCEPTED"
            || r[0][2].is_empty()
            || r[0][3] != self.binding
        {
            return Err(
                "PlayerReturnFire actual narrow assumption/decision/final scope missing".into(),
            );
        }
        let acceptance = match r[0][0] {
            "EXPLICIT_ASSUMPTION" => fire_audit::ReturnPolicyAcceptance::ExplicitAssumption,
            "SPECIFIC_DIFFERENCE" => fire_audit::ReturnPolicyAcceptance::SpecificDifference,
            _ => return Err("PlayerReturnFire no generic true/default policy is installed".into()),
        };
        Ok(fire_audit::ReturnPolicyDecision {
            decision_id: r[0][1].into(),
            explicit_model_assumption: r[0][2].into(),
            acceptance,
        })
    }
}
