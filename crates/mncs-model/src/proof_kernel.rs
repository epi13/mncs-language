//! RFC 0007 proof-core bootstrap: canonical artifacts, the independent
//! reference checker, and proof-bound evidence.
//!
//! The MNCS-native kernel lives in `library/core/proof_term.mncs` and
//! `library/core/proof_check.mncs`; this module is bootstrap debt by design:
//! canonical serialization, content identity, a second checker implementation
//! for differential validation, and the bridge that binds kernel verdicts to
//! compiler obligations. Every item here is inventoried in
//! `docs/rfc-0007-evidence.md` with a migration path toward MNCS.
//!
//! The reference checker encodes the same tranche rules as the MNCS kernel
//! (iterative tables in Rust versus a single forward pass in MNCS is the
//! deliberate implementation diversity; no code is shared between them).
//! Agreement is checked empirically over the shared corpora; it is evidence
//! of consistency, never a proof of checker correctness. Common-mode
//! dependencies that remain (SHA-256 identity, canonical JSON, and the MNCS
//! compiler itself for executed runs) are recorded in the evidence document.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};
use crate::{
    EvidenceAuthorityClass, EvidenceFreshness, IntegerOperation, ObligationStatus, SemanticId,
    VerifierIdentity, VerifierIndependence, VerifierMethod, VerifierRequest, VerifierResult,
};

pub const PROOF_KERNEL_ID: &str = "mncs:proof-kernel:0.1";
pub const PROOF_ARTIFACT_SCHEMA_VERSION: &str = "0.1";
pub const PROOF_MAX_UNIVERSE: i64 = 3;
pub const PROOF_BUFFER_CAPACITY: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofTag {
    Universe,
    Var,
    Pi,
    Lambda,
    Apply,
    Nat,
    Zero,
    Succ,
    Plus,
    NatElim,
    Eq,
    Refl,
    Unsupported,
}

impl ProofTag {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Universe" => Some(Self::Universe),
            "Var" => Some(Self::Var),
            "Pi" => Some(Self::Pi),
            "Lambda" => Some(Self::Lambda),
            "Apply" => Some(Self::Apply),
            "Nat" => Some(Self::Nat),
            "Zero" => Some(Self::Zero),
            "Succ" => Some(Self::Succ),
            "Plus" => Some(Self::Plus),
            "NatElim" => Some(Self::NatElim),
            "Eq" => Some(Self::Eq),
            "Refl" => Some(Self::Refl),
            "Unsupported" => Some(Self::Unsupported),
            _ => None,
        }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Universe => 1,
            Self::Var => 2,
            Self::Pi => 3,
            Self::Lambda => 4,
            Self::Apply => 5,
            Self::Nat => 6,
            Self::Zero => 7,
            Self::Succ => 8,
            Self::Plus => 9,
            Self::NatElim => 10,
            Self::Eq => 11,
            Self::Refl => 12,
            Self::Unsupported => 0,
        }
    }

    pub fn is_binder(self) -> bool {
        matches!(self, Self::Pi | Self::Lambda)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofCell {
    pub tag: ProofTag,
    pub args: [i64; 4],
}

impl ProofCell {
    pub fn new(tag: ProofTag, args: [i64; 4]) -> Self {
        Self { tag, args }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProofVerdict {
    Pass,
    Fail,
    Unknown,
}

impl ProofVerdict {
    pub fn dominate(self, other: Self) -> Self {
        match (self, other) {
            (Self::Fail, _) | (_, Self::Fail) => Self::Fail,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Pass,
        }
    }

    pub fn code(self) -> i64 {
        match self {
            Self::Pass => 0,
            Self::Fail => 1,
            Self::Unknown => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofArtifact {
    pub schema_version: String,
    pub identity: SemanticId,
    pub kernel: String,
    pub obligation: SemanticId,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: Vec<SemanticId>,
    pub cells: Vec<ProofCell>,
    pub count: usize,
    pub proof: usize,
    pub proposition: usize,
}

impl ProofArtifact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kernel: impl Into<String>,
        obligation: SemanticId,
        mut assumptions: Vec<SemanticId>,
        mut dependencies: Vec<SemanticId>,
        cells: Vec<ProofCell>,
        count: usize,
        proof: usize,
        proposition: usize,
    ) -> Self {
        assumptions.sort();
        assumptions.dedup();
        dependencies.sort();
        dependencies.dedup();
        let mut artifact = Self {
            schema_version: PROOF_ARTIFACT_SCHEMA_VERSION.to_owned(),
            identity: SemanticId(String::new()),
            kernel: kernel.into(),
            obligation,
            assumptions,
            dependencies,
            cells,
            count,
            proof,
            proposition,
        };
        artifact.seal();
        artifact
    }

    pub fn seal(&mut self) {
        self.identity = SemanticId(String::new());
        let canonical = canonical_json_value(self).expect("proof artifact is serializable");
        self.identity = SemanticId(format!("mncs:proof:{}", sha256_hex(canonical.as_bytes())));
    }

    pub fn identity_is_valid(&self) -> bool {
        if self.schema_version != PROOF_ARTIFACT_SCHEMA_VERSION || self.identity.0.is_empty() {
            return false;
        }
        let mut material = self.clone();
        material.identity = SemanticId(String::new());
        let canonical = canonical_json_value(&material).expect("proof artifact is serializable");
        self.identity == SemanticId(format!("mncs:proof:{}", sha256_hex(canonical.as_bytes())))
    }
}

/// A kernel verdict bound to the exact obligation, kernel version,
/// assumptions, and dependencies it was checked under. Reuse requires an
/// exact match on every field: any material change invalidates the binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofBinding {
    pub proof: SemanticId,
    pub kernel: String,
    pub obligation: SemanticId,
    pub assumptions: Vec<SemanticId>,
    pub dependencies: BTreeMap<SemanticId, String>,
    pub outcome: ProofVerdict,
}

impl ProofBinding {
    /// Bind a verified artifact. Returns `None` unless the independent
    /// reference checker returns `Pass` for the exact artifact bytes, so a
    /// binding always carries a checked verdict, never a claimed one.
    pub fn bind(
        artifact: &ProofArtifact,
        dependencies: BTreeMap<SemanticId, String>,
    ) -> Option<Self> {
        if !artifact.identity_is_valid() || artifact.kernel != PROOF_KERNEL_ID {
            return None;
        }
        if reference_check(artifact) != ProofVerdict::Pass {
            return None;
        }
        let mut assumptions = artifact.assumptions.clone();
        assumptions.sort();
        assumptions.dedup();
        Some(Self {
            proof: artifact.identity.clone(),
            kernel: artifact.kernel.clone(),
            obligation: artifact.obligation.clone(),
            assumptions,
            dependencies,
            outcome: ProofVerdict::Pass,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reusable_if(
        &self,
        proof: &SemanticId,
        kernel: &str,
        obligation: &SemanticId,
        assumptions: &[SemanticId],
        dependencies: &BTreeMap<SemanticId, String>,
        outcome: ProofVerdict,
    ) -> bool {
        let mut expected_assumptions = assumptions.to_vec();
        expected_assumptions.sort();
        expected_assumptions.dedup();
        self.proof == *proof
            && self.kernel == kernel
            && self.obligation == *obligation
            && self.assumptions == expected_assumptions
            && &self.dependencies == dependencies
            && self.outcome == outcome
            && self.outcome == ProofVerdict::Pass
    }
}

/// Per-cell tables of the reference pass. The layout mirrors the MNCS memo
/// field for field so differential review can compare rule outcomes index by
/// index; the code itself is an independent implementation of the same rules.
#[derive(Debug, Clone)]
struct ReferenceTables {
    rtag: Vec<i64>,
    rr: [Vec<i64>; 4],
    rep: Vec<i64>,
    tt: [Vec<i64>; 3],
    nval: Vec<i64>,
    mentions: Vec<i64>,
    status: ProofVerdict,
}

impl ReferenceTables {
    fn read(table: &[i64], raw: i64, ok: bool, fallback: i64) -> i64 {
        if ok {
            table[raw as usize]
        } else {
            fallback
        }
    }

    fn store_row(
        &mut self,
        index: usize,
        tag: i64,
        r: [i64; 4],
        rep: i64,
        nval: i64,
        mentions: i64,
    ) {
        self.rtag[index] = tag;
        for (slot, value) in r.into_iter().enumerate() {
            self.rr[slot][index] = value;
        }
        self.rep[index] = rep;
        self.nval[index] = nval;
        self.mentions[index] = mentions;
    }

    fn settle_row(&mut self, index: usize, t: [i64; 3], local: ProofVerdict) {
        self.tt[0][index] = t[0];
        self.tt[1][index] = t[1];
        self.tt[2][index] = t[2];
        self.status = self.status.dominate(local);
    }

    fn fresh(width: usize) -> Self {
        Self {
            rtag: vec![0; width],
            rr: [
                vec![0; width],
                vec![0; width],
                vec![0; width],
                vec![0; width],
            ],
            rep: vec![0; width],
            tt: [vec![0; width], vec![0; width], vec![0; width]],
            nval: vec![-1; width],
            mentions: vec![0; width],
            status: ProofVerdict::Pass,
        }
    }
}

fn valid_child(raw: i64, parent: usize, count: usize) -> Option<usize> {
    if raw >= 0 && (raw as usize) < parent && (raw as usize) < count {
        Some(raw as usize)
    } else {
        None
    }
}

/// Canonical meaning of a type cell: Nat -> (2,0,0), Universe(l) -> (1,l,0),
/// Pi(d,c) -> (3,d,c), Eq(T,_,_) -> (4,T,0); anything else is not a type.
fn denote(tag: i64, first: i64, second: i64) -> (i64, i64, i64) {
    match tag {
        6 => (2, 0, 0),
        1 => (1, first, 0),
        3 => (3, first, second),
        11 => (4, first, 0),
        _ => (0, 0, 0),
    }
}

fn advance_reference(
    tables: &mut ReferenceTables,
    cells: &[ProofCell],
    count: usize,
    index: usize,
) {
    let cell = &cells[index];
    let code = cell.tag.code();
    let self_index = index as i64;
    // `ReferenceTables::read` dereferences an already-computed table entry
    // for a validated child, or the fallback otherwise. Every rule below
    // gates trusted values on an explicit validity flag first.
    match code {
        1 => {
            let level = cell.args[0];
            let in_range = (0..=PROOF_MAX_UNIVERSE).contains(&level);
            let has_type = in_range && level < PROOF_MAX_UNIVERSE;
            let local = if !in_range || !has_type {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            tables.store_row(index, 1, [level, 0, 0, 0], self_index, -1, 0);
            let triple = if has_type {
                [1, level + 1, 0]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        6 => {
            tables.store_row(index, 6, [0, 0, 0, 0], self_index, -1, 0);
            tables.settle_row(index, [1, 0, 0], ProofVerdict::Pass);
        }
        7 => {
            tables.store_row(index, 7, [0, 0, 0, 0], self_index, 0, 0);
            tables.settle_row(index, [2, 0, 0], ProofVerdict::Pass);
        }
        2 => {
            let ok = valid_child(cell.args[0], index, count).is_some();
            let target = if ok { cell.args[0] } else { 0 };
            let binder = ReferenceTables::read(&tables.rtag, target, ok, 0);
            let bound = binder == 3 || binder == 4;
            let domain = ReferenceTables::read(&tables.rr[0], target, ok && bound, 0);
            let found = ok && bound;
            let denoted = denote(
                ReferenceTables::read(&tables.rtag, domain, found, 0),
                ReferenceTables::read(&tables.rr[0], domain, found, 0),
                ReferenceTables::read(&tables.rr[1], domain, found, 0),
            );
            let usable = denoted.0 != 0;
            let local = if !ok || !bound || !usable {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            tables.store_row(
                index,
                2,
                [if ok { target } else { 0 }, 0, 0, 0],
                self_index,
                -1,
                1,
            );
            let triple = if found && usable {
                [denoted.0, denoted.1, denoted.2]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        3 => {
            let ok_domain = valid_child(cell.args[0], index, count).is_some();
            let ok_codomain = valid_child(cell.args[1], index, count).is_some();
            let domain_kind = ReferenceTables::read(&tables.tt[0], cell.args[0], ok_domain, 0);
            let domain_level = ReferenceTables::read(&tables.tt[1], cell.args[0], ok_domain, 0);
            let codomain_kind = ReferenceTables::read(&tables.tt[0], cell.args[1], ok_codomain, 0);
            let codomain_level = ReferenceTables::read(&tables.tt[1], cell.args[1], ok_codomain, 0);
            let well_typed = ok_domain && ok_codomain && domain_kind == 1 && codomain_kind == 1;
            let joined = domain_level.max(codomain_level);
            let local = if !well_typed || joined > PROOF_MAX_UNIVERSE {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let domain_rep =
                ReferenceTables::read(&tables.rep, cell.args[0], ok_domain, self_index);
            let codomain_rep =
                ReferenceTables::read(&tables.rep, cell.args[1], ok_codomain, self_index);
            let mentioned = i64::from(
                ReferenceTables::read(&tables.mentions, cell.args[0], ok_domain, 0)
                    + ReferenceTables::read(&tables.mentions, cell.args[1], ok_codomain, 0)
                    > 0,
            );
            tables.store_row(
                index,
                3,
                [domain_rep, codomain_rep, 0, 0],
                self_index,
                -1,
                mentioned,
            );
            let triple = if local == ProofVerdict::Pass {
                [1, joined, 0]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        4 => {
            let ok_domain = valid_child(cell.args[0], index, count).is_some();
            let ok_body = valid_child(cell.args[1], index, count).is_some();
            let ok_claim = valid_child(cell.args[2], index, count).is_some();
            let domain_is_type =
                ReferenceTables::read(&tables.tt[0], cell.args[0], ok_domain, 0) == 1;
            let claim_is_pi = ReferenceTables::read(&tables.rtag, cell.args[2], ok_claim, 0) == 3;
            let claim_domain =
                ReferenceTables::read(&tables.rr[0], cell.args[2], ok_claim && claim_is_pi, 0);
            let claim_codomain =
                ReferenceTables::read(&tables.rr[1], cell.args[2], ok_claim && claim_is_pi, 0);
            let domain_rep =
                ReferenceTables::read(&tables.rep, cell.args[0], ok_domain, self_index);
            let actual = (
                ReferenceTables::read(&tables.tt[0], cell.args[1], ok_body, 0),
                ReferenceTables::read(&tables.tt[1], cell.args[1], ok_body, 0),
                ReferenceTables::read(&tables.tt[2], cell.args[1], ok_body, 0),
            );
            let settled_claim = ok_claim && claim_is_pi;
            let claimed = denote(
                ReferenceTables::read(&tables.rtag, claim_codomain, settled_claim, 0),
                ReferenceTables::read(&tables.rr[0], claim_codomain, settled_claim, 0),
                ReferenceTables::read(&tables.rr[1], claim_codomain, settled_claim, 0),
            );
            let refs_ok = ok_domain && ok_body && ok_claim;
            let local = if !refs_ok
                || !domain_is_type
                || !claim_is_pi
                || claim_domain != domain_rep
                || actual != claimed
            {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let body_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_body, self_index);
            let mentioned = ReferenceTables::read(&tables.mentions, cell.args[1], ok_body, 0);
            tables.store_row(
                index,
                4,
                [domain_rep, body_rep, 0, 0],
                self_index,
                -1,
                mentioned,
            );
            let triple = if local == ProofVerdict::Pass {
                [3, claim_domain, claim_codomain]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        5 => {
            let ok_fun = valid_child(cell.args[0], index, count).is_some();
            let ok_arg = valid_child(cell.args[1], index, count).is_some();
            let fun_kind = ReferenceTables::read(&tables.tt[0], cell.args[0], ok_fun, 0);
            let domain = ReferenceTables::read(&tables.tt[1], cell.args[0], ok_fun, 0);
            let codomain = ReferenceTables::read(&tables.tt[2], cell.args[0], ok_fun, 0);
            let is_pi = fun_kind == 3;
            let usable = ok_fun && is_pi;
            let required = denote(
                ReferenceTables::read(&tables.rtag, domain, usable, 0),
                ReferenceTables::read(&tables.rr[0], domain, usable, 0),
                ReferenceTables::read(&tables.rr[1], domain, usable, 0),
            );
            let actual = (
                ReferenceTables::read(&tables.tt[0], cell.args[1], ok_arg, 0),
                ReferenceTables::read(&tables.tt[1], cell.args[1], ok_arg, 0),
                ReferenceTables::read(&tables.tt[2], cell.args[1], ok_arg, 0),
            );
            let applicable = ok_fun && ok_arg && is_pi && actual == required;
            let dependent = ReferenceTables::read(&tables.mentions, codomain, usable, 0) > 0;
            let local = if !applicable {
                ProofVerdict::Fail
            } else if dependent {
                ProofVerdict::Unknown
            } else {
                ProofVerdict::Pass
            };
            let produced = denote(
                ReferenceTables::read(&tables.rtag, codomain, usable, 0),
                ReferenceTables::read(&tables.rr[0], codomain, usable, 0),
                ReferenceTables::read(&tables.rr[1], codomain, usable, 0),
            );
            let fun_rep = ReferenceTables::read(&tables.rep, cell.args[0], ok_fun, self_index);
            let arg_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_arg, self_index);
            let mentioned = i64::from(
                ReferenceTables::read(&tables.mentions, cell.args[0], ok_fun, 0)
                    + ReferenceTables::read(&tables.mentions, cell.args[1], ok_arg, 0)
                    > 0,
            );
            tables.store_row(
                index,
                5,
                [fun_rep, arg_rep, 0, 0],
                self_index,
                -1,
                mentioned,
            );
            let triple = if local == ProofVerdict::Pass {
                [produced.0, produced.1, produced.2]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        8 => {
            let ok = valid_child(cell.args[0], index, count).is_some();
            let is_nat = ReferenceTables::read(&tables.tt[0], cell.args[0], ok, 0) == 2;
            let local = if !ok || !is_nat {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let pred_rep = ReferenceTables::read(&tables.rep, cell.args[0], ok, self_index);
            let pred_value = ReferenceTables::read(&tables.nval, cell.args[0], ok, -1);
            let value = if ok && is_nat && pred_value >= 0 {
                pred_value + 1
            } else {
                -1
            };
            let mentioned = ReferenceTables::read(&tables.mentions, cell.args[0], ok, 0);
            tables.store_row(index, 8, [pred_rep, 0, 0, 0], self_index, value, mentioned);
            let triple = if local == ProofVerdict::Pass {
                [2, 0, 0]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        9 => {
            let ok_left = valid_child(cell.args[0], index, count).is_some();
            let ok_right = valid_child(cell.args[1], index, count).is_some();
            let both_nat = ReferenceTables::read(&tables.tt[0], cell.args[0], ok_left, 0) == 2
                && ReferenceTables::read(&tables.tt[0], cell.args[1], ok_right, 0) == 2;
            let local = if !ok_left || !ok_right || !both_nat {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let left_value = ReferenceTables::read(&tables.nval, cell.args[0], ok_left, -1);
            let right_value = ReferenceTables::read(&tables.nval, cell.args[1], ok_right, -1);
            let value = if ok_left && ok_right && both_nat && left_value >= 0 && right_value >= 0 {
                left_value + right_value
            } else {
                -1
            };
            let left_rep = ReferenceTables::read(&tables.rep, cell.args[0], ok_left, self_index);
            let right_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_right, self_index);
            let mentioned = i64::from(
                ReferenceTables::read(&tables.mentions, cell.args[0], ok_left, 0)
                    + ReferenceTables::read(&tables.mentions, cell.args[1], ok_right, 0)
                    > 0,
            );
            tables.store_row(
                index,
                9,
                [left_rep, right_rep, 0, 0],
                self_index,
                value,
                mentioned,
            );
            let triple = if local == ProofVerdict::Pass {
                [2, 0, 0]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        10 => {
            let present = [0, 1, 2, 3]
                .into_iter()
                .map(|slot| valid_child(cell.args[slot], index, count).is_some())
                .collect::<Vec<_>>();
            let (ok_motive, ok_zero, ok_succ, ok_target) =
                (present[0], present[1], present[2], present[3]);
            let motive_is_pi =
                ReferenceTables::read(&tables.tt[0], cell.args[0], ok_motive, 0) == 3;
            let motive_ok = ok_motive && motive_is_pi;
            let motive_domain = ReferenceTables::read(&tables.tt[1], cell.args[0], motive_ok, 0);
            let motive_codomain = ReferenceTables::read(&tables.tt[2], cell.args[0], motive_ok, 0);
            let domain_denotation = denote(
                ReferenceTables::read(&tables.rtag, motive_domain, motive_ok, 0),
                ReferenceTables::read(&tables.rr[0], motive_domain, motive_ok, 0),
                ReferenceTables::read(&tables.rr[1], motive_domain, motive_ok, 0),
            );
            let cod_denotation = denote(
                ReferenceTables::read(&tables.rtag, motive_codomain, motive_ok, 0),
                ReferenceTables::read(&tables.rr[0], motive_codomain, motive_ok, 0),
                ReferenceTables::read(&tables.rr[1], motive_codomain, motive_ok, 0),
            );
            let motive_well_formed = motive_ok && domain_denotation.0 == 2 && cod_denotation.0 == 1;
            let dependent =
                ReferenceTables::read(&tables.mentions, motive_codomain, motive_well_formed, 0) > 0;
            let zero_actual = (
                ReferenceTables::read(&tables.tt[0], cell.args[1], ok_zero, 0),
                ReferenceTables::read(&tables.tt[1], cell.args[1], ok_zero, 0),
                ReferenceTables::read(&tables.tt[2], cell.args[1], ok_zero, 0),
            );
            let succ_is_pi = ReferenceTables::read(&tables.tt[0], cell.args[2], ok_succ, 0) == 3;
            let succ_domain = ReferenceTables::read(&tables.tt[1], cell.args[2], ok_succ, 0);
            let succ_codomain = ReferenceTables::read(&tables.tt[2], cell.args[2], ok_succ, 0);
            let succ_domain_denotation = denote(
                ReferenceTables::read(&tables.rtag, succ_domain, ok_succ && succ_is_pi, 0),
                ReferenceTables::read(&tables.rr[0], succ_domain, ok_succ && succ_is_pi, 0),
                ReferenceTables::read(&tables.rr[1], succ_domain, ok_succ && succ_is_pi, 0),
            );
            let succ_cod_denotation = denote(
                ReferenceTables::read(&tables.rtag, succ_codomain, ok_succ && succ_is_pi, 0),
                ReferenceTables::read(&tables.rr[0], succ_codomain, ok_succ && succ_is_pi, 0),
                ReferenceTables::read(&tables.rr[1], succ_codomain, ok_succ && succ_is_pi, 0),
            );
            let target_is_nat =
                ReferenceTables::read(&tables.tt[0], cell.args[3], ok_target, 0) == 2;
            let refs_ok = ok_motive && ok_zero && ok_succ && ok_target;
            let branches_ok = motive_well_formed
                && zero_actual == cod_denotation
                && succ_is_pi
                && succ_domain_denotation.0 == 2
                && succ_cod_denotation == cod_denotation
                && target_is_nat;
            let target_value = ReferenceTables::read(&tables.nval, cell.args[3], ok_target, -1);
            let local = if !refs_ok || !motive_well_formed || !branches_ok {
                ProofVerdict::Fail
            } else if dependent || target_value != 0 {
                ProofVerdict::Unknown
            } else {
                ProofVerdict::Pass
            };
            // Closed-zero iota: the eliminator takes the zero branch's
            // representative, value, and descriptor rows.
            let settled = local == ProofVerdict::Pass;
            let pick = |primary: i64, fallback: i64| -> i64 {
                if settled {
                    primary
                } else {
                    fallback
                }
            };
            let motive_rep =
                ReferenceTables::read(&tables.rep, cell.args[0], ok_motive, self_index);
            let zero_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_zero, self_index);
            let succ_rep = ReferenceTables::read(&tables.rep, cell.args[2], ok_succ, self_index);
            let target_rep =
                ReferenceTables::read(&tables.rep, cell.args[3], ok_target, self_index);
            tables.store_row(
                index,
                10,
                [
                    pick(zero_rep, motive_rep),
                    pick(0, zero_rep),
                    pick(0, succ_rep),
                    pick(0, target_rep),
                ],
                pick(zero_rep, self_index),
                pick(
                    ReferenceTables::read(&tables.nval, cell.args[1], ok_zero, -1),
                    -1,
                ),
                pick(
                    ReferenceTables::read(&tables.mentions, cell.args[1], ok_zero, 0),
                    0,
                ),
            );
            let triple = if settled {
                [zero_actual.0, zero_actual.1, zero_actual.2]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        11 => {
            let ok_carrier = valid_child(cell.args[0], index, count).is_some();
            let ok_left = valid_child(cell.args[1], index, count).is_some();
            let ok_right = valid_child(cell.args[2], index, count).is_some();
            let carrier_is_type =
                ReferenceTables::read(&tables.tt[0], cell.args[0], ok_carrier, 0) == 1;
            let carrier_level = ReferenceTables::read(&tables.tt[1], cell.args[0], ok_carrier, 0);
            let carrier_rep =
                ReferenceTables::read(&tables.rep, cell.args[0], ok_carrier, self_index);
            let required = denote(
                ReferenceTables::read(&tables.rtag, carrier_rep, ok_carrier, 0),
                ReferenceTables::read(&tables.rr[0], carrier_rep, ok_carrier, 0),
                ReferenceTables::read(&tables.rr[1], carrier_rep, ok_carrier, 0),
            );
            let left_actual = (
                ReferenceTables::read(&tables.tt[0], cell.args[1], ok_left, 0),
                ReferenceTables::read(&tables.tt[1], cell.args[1], ok_left, 0),
                ReferenceTables::read(&tables.tt[2], cell.args[1], ok_left, 0),
            );
            let right_actual = (
                ReferenceTables::read(&tables.tt[0], cell.args[2], ok_right, 0),
                ReferenceTables::read(&tables.tt[1], cell.args[2], ok_right, 0),
                ReferenceTables::read(&tables.tt[2], cell.args[2], ok_right, 0),
            );
            let refs_ok = ok_carrier && ok_left && ok_right;
            let local = if !refs_ok
                || !carrier_is_type
                || left_actual != required
                || right_actual != required
            {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let left_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_left, self_index);
            let right_rep = ReferenceTables::read(&tables.rep, cell.args[2], ok_right, self_index);
            let mentioned = i64::from(
                ReferenceTables::read(&tables.mentions, cell.args[1], ok_left, 0)
                    + ReferenceTables::read(&tables.mentions, cell.args[2], ok_right, 0)
                    > 0,
            );
            tables.store_row(
                index,
                11,
                [carrier_rep, left_rep, right_rep, 0],
                self_index,
                -1,
                mentioned,
            );
            let triple = if local == ProofVerdict::Pass {
                [1, carrier_level, 0]
            } else {
                [0, 0, 0]
            };
            tables.settle_row(index, triple, local);
        }
        12 => {
            let ok_witness = valid_child(cell.args[0], index, count).is_some();
            let ok_claim = valid_child(cell.args[1], index, count).is_some();
            let claim_is_eq = ReferenceTables::read(&tables.rtag, cell.args[1], ok_claim, 0) == 11;
            let local = if !ok_witness || !ok_claim || !claim_is_eq {
                ProofVerdict::Fail
            } else {
                ProofVerdict::Pass
            };
            let witness_rep =
                ReferenceTables::read(&tables.rep, cell.args[0], ok_witness, self_index);
            let claim_rep = ReferenceTables::read(&tables.rep, cell.args[1], ok_claim, self_index);
            let mentioned = i64::from(
                ReferenceTables::read(&tables.mentions, cell.args[0], ok_witness, 0)
                    + ReferenceTables::read(&tables.mentions, cell.args[1], ok_claim, 0)
                    > 0,
            );
            tables.store_row(
                index,
                12,
                [witness_rep, claim_rep, 0, 0],
                self_index,
                -1,
                mentioned,
            );
            tables.settle_row(index, [0, 0, 0], local);
        }
        _ => {
            tables.store_row(index, 0, [0, 0, 0, 0], self_index, -1, 0);
            tables.settle_row(index, [0, 0, 0], ProofVerdict::Unknown);
        }
    }
}

/// Deterministically check a proof artifact: run the forward pass over the
/// active prefix, then verify that `proof` inhabits the proposition denoted
/// by `proposition`. Refl proofs are verified against their Eq proposition;
/// every other proof term must synthesize exactly the denoted descriptor.
/// Open proof terms and out-of-range claims fail closed. Undecidable
/// structure always resolves to `Unknown`, never `Pass`.
pub fn reference_check(artifact: &ProofArtifact) -> ProofVerdict {
    if artifact.cells.len() > PROOF_BUFFER_CAPACITY {
        return ProofVerdict::Fail;
    }
    if artifact.count > artifact.cells.len() {
        return ProofVerdict::Fail;
    }
    let count = artifact.count;
    let mut tables = ReferenceTables::fresh(count.max(1));
    for index in 0..count {
        advance_reference(&mut tables, &artifact.cells, count, index);
    }
    let below = |raw: usize| raw < count;
    let indices_ok = below(artifact.proof) && below(artifact.proposition);
    let rep_of = |raw: usize, ok: bool| -> usize {
        if ok {
            tables.rep[raw] as usize
        } else {
            0
        }
    };
    let prop_rep = rep_of(artifact.proposition, below(artifact.proposition));
    let prop_denotation = denote(
        tables.rtag[prop_rep],
        tables.rr[0][prop_rep],
        tables.rr[1][prop_rep],
    );
    let prop_is_type = prop_denotation.0 != 0 && indices_ok;
    let proof_desc = (
        tables.tt[0][artifact.proof.min(count.max(1) - 1)],
        tables.tt[1][artifact.proof.min(count.max(1) - 1)],
        tables.tt[2][artifact.proof.min(count.max(1) - 1)],
    );
    let proof_tag = tables.rtag[rep_of(artifact.proof, below(artifact.proof))];
    let is_refl = proof_tag == 12 && indices_ok;
    let witness = tables.rr[0][rep_of(artifact.proof, below(artifact.proof))] as usize;
    let equation = tables.rr[1][rep_of(artifact.proof, below(artifact.proof))] as usize;
    let in_equation = is_refl && equation < count && witness < count;
    let eq_carrier = if in_equation {
        tables.rr[0][equation] as usize
    } else {
        0
    };
    let eq_left = if in_equation {
        tables.rr[1][equation] as usize
    } else {
        0
    };
    let eq_right = if in_equation {
        tables.rr[2][equation] as usize
    } else {
        0
    };
    let sides_defined = in_equation && eq_carrier < count && eq_left < count && eq_right < count;
    // Definitional equality of the Refl sides: shared representatives, or
    // equal closed-literal values on both sides.
    let side_matches = |side: usize| -> bool {
        if !sides_defined {
            return false;
        }
        tables.rep[side] == tables.rep[witness]
            || (tables.nval[side] >= 0
                && tables.nval[witness] >= 0
                && tables.nval[side] == tables.nval[witness])
    };
    let witness_desc = if in_equation {
        (
            tables.tt[0][witness],
            tables.tt[1][witness],
            tables.tt[2][witness],
        )
    } else {
        (0, 0, 0)
    };
    let carrier_denotation = if sides_defined {
        denote(
            tables.rtag[eq_carrier],
            tables.rr[0][eq_carrier],
            tables.rr[1][eq_carrier],
        )
    } else {
        (0, 0, 0)
    };
    let refl_holds = in_equation
        && sides_defined
        && equation == prop_rep
        && side_matches(eq_left)
        && side_matches(eq_right)
        && witness_desc == carrier_denotation;
    let descriptor_matches = indices_ok && proof_desc == prop_denotation;
    let claim_holds = if is_refl {
        refl_holds
    } else {
        descriptor_matches
    };
    let open = indices_ok && tables.mentions[artifact.proof] > 0;
    let mut verdict = tables.status;
    if !indices_ok || !prop_is_type || !claim_holds {
        verdict = verdict.dominate(ProofVerdict::Fail);
    }
    if open {
        verdict = verdict.dominate(ProofVerdict::Unknown);
    }
    verdict
}

/// Build the verifier-level result for an exact constant integer operation
/// covered by a kernel proof binding. The result is `Pass` only when the
/// binding is exactly reusable for the requested obligation, the kernel
/// version matches, the subjects line up, and the exact machine-integer
/// evaluation reports no overflow. Every other case fails closed.
pub fn kernel_backed_range_result(
    request: &VerifierRequest,
    operation: &IntegerOperation,
    binding: &ProofBinding,
) -> VerifierResult {
    let kernel_identity = VerifierIdentity {
        identity: SemanticId(format!("{PROOF_KERNEL_ID}:reference")),
        name: "mncs-proof-kernel".to_owned(),
        version: "0.1".to_owned(),
        independence: VerifierIndependence::IndependentImplementation,
    };
    let mut result = VerifierResult {
        schema_version: crate::verifier::VERIFIER_SCHEMA_VERSION.to_owned(),
        obligation: request.obligation.identity.clone(),
        subject: request.subject.clone(),
        scope: request.scope.clone(),
        status: ObligationStatus::Fail,
        verifier: kernel_identity,
        authority: EvidenceAuthorityClass::KernelProof,
        method: VerifierMethod::KernelProof,
        assumptions: request.assumptions.clone(),
        dependencies: request.dependencies.clone(),
        dependency_fingerprints: request.dependency_fingerprints.clone(),
        artifact: Some(binding.proof.0.clone()),
        limitations: vec![
            "kernel proof covers closed Nat computation identity only".to_owned(),
            "machine-integer range established by exact constant evaluation, not by the Nat proof alone".to_owned(),
            "valid only for the bound obligation, kernel version, assumptions, and dependencies".to_owned(),
        ],
        freshness: EvidenceFreshness::Current,
    };
    let binding_ok = binding.reusable_if(
        &binding.proof,
        PROOF_KERNEL_ID,
        &request.obligation.identity,
        &request.assumptions,
        &request.dependency_fingerprints,
        ProofVerdict::Pass,
    ) && binding.dependencies == request.dependency_fingerprints
        && request.obligation.subject == request.subject;
    if !binding_ok {
        return result;
    }
    let evaluation = operation.evaluate();
    if evaluation.overflow || evaluation.value.is_none() {
        return result;
    }
    result.status = ObligationStatus::Pass;
    result
}

/// One experiment-corpus case decoded into kernel inputs plus the expected
/// verdict. This is test support shared by the differential suite: the MNCS
/// execution corpus stays the single source of truth for both checkers.
#[derive(Debug, Clone)]
pub struct ProofCorpusCase {
    pub id: String,
    pub cells: Vec<ProofCell>,
    pub count: usize,
    pub proof: usize,
    pub proposition: usize,
    /// Pinned expectation for curated cases; `None` for fuzz cases, where
    /// the reference checker itself is the oracle.
    pub expected: Option<ProofVerdict>,
}

fn corpus_integer(value: &serde_json::Value) -> Option<i64> {
    value.get("integer")?.get("value")?.as_i64()
}

fn corpus_byte(value: &serde_json::Value) -> Option<usize> {
    let byte = value.get("byte")?.get("value")?.as_u64()?;
    usize::try_from(byte).ok()
}

fn corpus_cell(value: &serde_json::Value) -> Option<ProofCell> {
    let record = value.get("record")?;
    let mut args = [0i64; 4];
    let mut tag = None;
    for field in record.get("fields")?.as_array()? {
        let pair = field.as_array()?;
        let name = pair.first()?.as_str()?;
        let item = pair.get(1)?;
        match name {
            "arg0" | "arg1" | "arg2" | "arg3" => {
                let slot = (name.as_bytes()[3] - b'0') as usize;
                args[slot] = corpus_integer(item)?;
            }
            "tag" => {
                let variant = item.get("finite")?.get("variant_identity")?.as_str()?;
                let short = variant.rsplit("::").next()?;
                tag = ProofTag::from_name(short);
            }
            _ => return None,
        }
    }
    Some(ProofCell::new(tag?, args))
}

fn verdict_from_code(code: i64) -> Option<ProofVerdict> {
    match code {
        0 => Some(ProofVerdict::Pass),
        1 => Some(ProofVerdict::Fail),
        2 => Some(ProofVerdict::Unknown),
        _ => None,
    }
}

pub fn parse_proof_corpus(text: &str) -> Result<Vec<ProofCorpusCase>, String> {
    let document: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("corpus JSON: {error}"))?;
    let cases = document
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "corpus has no cases array".to_owned())?;
    let mut parsed = Vec::with_capacity(cases.len());
    for case in cases {
        let id = case
            .get("id")
            .or_else(|| case.get("case_id"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "case has no id".to_owned())?
            .to_owned();
        let arguments = case
            .get("request")
            .and_then(|request| request.get("arguments"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{id}: no request arguments"))?;
        let values = arguments
            .first()
            .and_then(|first| first.get("sequence"))
            .and_then(|sequence| sequence.get("values"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{id}: no sequence values"))?;
        if values.len() != PROOF_BUFFER_CAPACITY {
            return Err(format!("{id}: buffer is not {PROOF_BUFFER_CAPACITY} cells"));
        }
        let mut cells = Vec::with_capacity(values.len());
        for value in values {
            cells.push(corpus_cell(value).ok_or_else(|| format!("{id}: bad cell"))?);
        }
        let count = corpus_byte(&arguments[1]).ok_or_else(|| format!("{id}: bad count"))?;
        let proof = corpus_byte(&arguments[2]).ok_or_else(|| format!("{id}: bad proof"))?;
        let proposition = corpus_byte(&arguments[3]).ok_or_else(|| format!("{id}: bad prop"))?;
        let expected = case
            .get("expected")
            .and_then(serde_json::Value::as_array)
            .and_then(|expected| expected.first())
            .and_then(corpus_integer)
            .and_then(verdict_from_code);
        parsed.push(ProofCorpusCase {
            id,
            cells,
            count,
            proof,
            proposition,
            expected,
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::tests::valid_program;

    fn test_cells() -> Vec<ProofCell> {
        // Eq Nat Zero Zero by Refl, padded to the buffer capacity.
        let mut cells = vec![
            ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]),
            ProofCell::new(ProofTag::Zero, [0, 0, 0, 0]),
            ProofCell::new(ProofTag::Eq, [0, 1, 1, 0]),
            ProofCell::new(ProofTag::Refl, [1, 2, 0, 0]),
        ];
        while cells.len() < PROOF_BUFFER_CAPACITY {
            cells.push(ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]));
        }
        cells
    }

    fn test_artifact(
        cells: Vec<ProofCell>,
        count: usize,
        proof: usize,
        proposition: usize,
    ) -> ProofArtifact {
        ProofArtifact::new(
            PROOF_KERNEL_ID,
            SemanticId("mncs:test:obligation".to_owned()),
            Vec::new(),
            vec![SemanticId("mncs:test:dependency".to_owned())],
            cells,
            count,
            proof,
            proposition,
        )
    }

    #[test]
    fn reference_agrees_with_checked_in_corpus_expectations() {
        let path = format!(
            "{}/../../examples/execution/proof-kernel-corpus.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).expect("proof kernel corpus");
        let cases = parse_proof_corpus(&text).expect("parse proof kernel corpus");
        assert!(!cases.is_empty());
        for case in &cases {
            let artifact = ProofArtifact::new(
                PROOF_KERNEL_ID,
                SemanticId("mncs:test:obligation".to_owned()),
                Vec::new(),
                Vec::new(),
                case.cells.clone(),
                case.count,
                case.proof,
                case.proposition,
            );
            if let Some(expected) = case.expected {
                assert_eq!(
                    reference_check(&artifact),
                    expected,
                    "reference diverges on corpus case {}",
                    case.id
                );
            }
        }
    }

    #[test]
    fn reference_accepts_minimal_closed_theorem() {
        let artifact = test_artifact(test_cells(), 4, 3, 2);
        assert!(artifact.identity_is_valid());
        assert_eq!(reference_check(&artifact), ProofVerdict::Pass);
    }

    #[test]
    fn reference_computes_closed_plus() {
        // 2 + 3 = 5 witnessed by Refl over a kernel-computed Plus.
        let mut cells = vec![
            ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]),  // 0
            ProofCell::new(ProofTag::Zero, [0, 0, 0, 0]), // 1
            ProofCell::new(ProofTag::Succ, [1, 0, 0, 0]), // 2
            ProofCell::new(ProofTag::Succ, [2, 0, 0, 0]), // 3
            ProofCell::new(ProofTag::Succ, [3, 0, 0, 0]), // 4
            ProofCell::new(ProofTag::Succ, [4, 0, 0, 0]), // 5
            ProofCell::new(ProofTag::Succ, [5, 0, 0, 0]), // 6
            ProofCell::new(ProofTag::Plus, [3, 4, 0, 0]), // 7
            ProofCell::new(ProofTag::Eq, [0, 7, 6, 0]),   // 8
            ProofCell::new(ProofTag::Refl, [7, 8, 0, 0]), // 9
        ];
        while cells.len() < PROOF_BUFFER_CAPACITY {
            cells.push(ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]));
        }
        let artifact = test_artifact(cells, 10, 9, 8);
        assert_eq!(reference_check(&artifact), ProofVerdict::Pass);
    }

    #[test]
    fn reference_rejects_wrong_equality_sides() {
        let mut cells = test_cells();
        cells[2] = ProofCell::new(ProofTag::Eq, [0, 1, 1, 0]);
        cells[3] = ProofCell::new(ProofTag::Refl, [1, 2, 0, 0]);
        cells.insert(2, ProofCell::new(ProofTag::Succ, [1, 0, 0, 0]));
        cells.pop();
        // Buffer: Nat, Zero, Succ(Zero), Eq Nat Zero (Succ Zero), Refl(Zero).
        cells[3] = ProofCell::new(ProofTag::Eq, [0, 1, 2, 0]);
        cells[4] = ProofCell::new(ProofTag::Refl, [1, 3, 0, 0]);
        let artifact = test_artifact(cells, 5, 4, 3);
        assert_eq!(reference_check(&artifact), ProofVerdict::Fail);
    }

    #[test]
    fn reference_rejects_forward_references_and_cycles() {
        let mut cells = test_cells();
        cells[1] = ProofCell::new(ProofTag::Succ, [5, 0, 0, 0]);
        let artifact = test_artifact(cells, 2, 1, 0);
        assert_eq!(reference_check(&artifact), ProofVerdict::Fail);
    }

    #[test]
    fn reference_rejects_universe_ceiling_breach() {
        let mut cells = test_cells();
        cells[0] = ProofCell::new(ProofTag::Universe, [3, 0, 0, 0]);
        let artifact = test_artifact(vec![cells[0].clone()], 1, 0, 0);
        assert_eq!(reference_check(&artifact), ProofVerdict::Fail);
    }

    #[test]
    fn reference_holds_unknown_for_unsupported_terms() {
        let mut cells = test_cells();
        cells[1] = ProofCell::new(ProofTag::Unsupported, [0, 0, 0, 0]);
        let artifact = test_artifact(cells, 4, 3, 2);
        // The unsupported cell poisons the pass, but the mismatched claim
        // also fails: FAIL dominates UNKNOWN in the join.
        assert_eq!(reference_check(&artifact), ProofVerdict::Fail);
    }

    #[test]
    fn reference_holds_pure_unknown_for_open_dependent_claim() {
        // Dependent lambda over an Eq motive: per-cell rules pass, the open
        // proof term keeps the verdict at UNKNOWN.
        let mut cells = vec![
            ProofCell::new(ProofTag::Universe, [0, 0, 0, 0]), // 0
            ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]),      // 1
            ProofCell::new(ProofTag::Pi, [1, 1, 0, 0]),       // 2
            ProofCell::new(ProofTag::Var, [2, 0, 0, 0]),      // 3
            ProofCell::new(ProofTag::Eq, [1, 3, 3, 0]),       // 4
            ProofCell::new(ProofTag::Pi, [4, 4, 0, 0]),       // 5
            ProofCell::new(ProofTag::Var, [5, 0, 0, 0]),      // 6
            ProofCell::new(ProofTag::Lambda, [4, 6, 5, 0]),   // 7
        ];
        while cells.len() < PROOF_BUFFER_CAPACITY {
            cells.push(ProofCell::new(ProofTag::Nat, [0, 0, 0, 0]));
        }
        let artifact = test_artifact(cells, 8, 7, 5);
        assert_eq!(reference_check(&artifact), ProofVerdict::Unknown);
    }

    #[test]
    fn artifact_identity_tampering_fails_closed() {
        let mut artifact = test_artifact(test_cells(), 4, 3, 2);
        artifact.cells[1] = ProofCell::new(ProofTag::Succ, [1, 0, 0, 0]);
        assert!(!artifact.identity_is_valid());
        assert!(ProofBinding::bind(&artifact, BTreeMap::new()).is_none());
    }

    #[test]
    fn binding_requires_exact_kernel_and_pass() {
        let artifact = test_artifact(test_cells(), 4, 3, 2);
        let dependencies = BTreeMap::from([(
            SemanticId("mncs:test:dependency".to_owned()),
            "fingerprint".to_owned(),
        )]);
        let binding = ProofBinding::bind(&artifact, dependencies.clone()).expect("bind");
        assert!(binding.reusable_if(
            &artifact.identity,
            PROOF_KERNEL_ID,
            &artifact.obligation,
            &[],
            &dependencies,
            ProofVerdict::Pass,
        ));
        // Wrong kernel version invalidates reuse.
        assert!(!binding.reusable_if(
            &artifact.identity,
            "mncs:proof-kernel:9.9",
            &artifact.obligation,
            &[],
            &dependencies,
            ProofVerdict::Pass,
        ));
        // Changed obligation identity invalidates reuse.
        assert!(!binding.reusable_if(
            &artifact.identity,
            PROOF_KERNEL_ID,
            &SemanticId("mncs:test:other-obligation".to_owned()),
            &[],
            &dependencies,
            ProofVerdict::Pass,
        ));
        // Changed dependency fingerprints invalidate reuse.
        let changed = BTreeMap::from([(
            SemanticId("mncs:test:dependency".to_owned()),
            "changed".to_owned(),
        )]);
        assert!(!binding.reusable_if(
            &artifact.identity,
            PROOF_KERNEL_ID,
            &artifact.obligation,
            &[],
            &changed,
            ProofVerdict::Pass,
        ));
    }

    #[test]
    fn kernel_backed_range_result_is_fail_closed() {
        use crate::{ArithmeticIntent, IntegerOperation, IntegerType};
        let operation = IntegerOperation {
            operator: "add".to_owned(),
            operand_type: IntegerType {
                bits: 64,
                signed: true,
            },
            left: 2,
            right: 3,
            intent: ArithmeticIntent::Checked,
        };
        let subject = operation.identity();
        let obligation = crate::ObligationRecord {
            schema_version: crate::OBLIGATION_SCHEMA_VERSION.to_owned(),
            identity: SemanticId("mncs:test:obligation".to_owned()),
            subject: subject.clone(),
            requirement: SemanticId("requirement:no-overflow".to_owned()),
            status: ObligationStatus::Unknown,
            method: "test".to_owned(),
            assumptions: Vec::new(),
            dependencies: vec![subject.clone()],
            freshness: EvidenceFreshness::Unknown,
            fallback: None,
        };
        let identities = valid_program().semantic_identities();
        let _ = identities;
        let fingerprints = BTreeMap::from([(subject.clone(), "operation".to_owned())]);
        let request = crate::VerifierRequest {
            schema_version: crate::verifier::VERIFIER_SCHEMA_VERSION.to_owned(),
            obligation: obligation.clone(),
            subject: subject.clone(),
            scope: "test".to_owned(),
            input: crate::VerifierInput::Capability(crate::CapabilityVerifierInput {
                authorized: None,
            }),
            assumptions: Vec::new(),
            dependencies: vec![subject.clone()],
            dependency_fingerprints: fingerprints.clone(),
        };
        let artifact = ProofArtifact::new(
            PROOF_KERNEL_ID,
            obligation.identity.clone(),
            Vec::new(),
            vec![subject.clone()],
            test_cells(),
            4,
            3,
            2,
        );
        let binding = ProofBinding::bind(&artifact, fingerprints.clone()).expect("bind test proof");
        let result = kernel_backed_range_result(&request, &operation, &binding);
        assert_eq!(result.status, ObligationStatus::Pass);
        assert_eq!(result.authority, EvidenceAuthorityClass::KernelProof);
        assert_eq!(result.method, VerifierMethod::KernelProof);
        // A mismatched obligation fails closed even with a valid binding.
        let mut wrong = request.clone();
        wrong.obligation.identity = SemanticId("mncs:test:other".to_owned());
        let denied = kernel_backed_range_result(&wrong, &operation, &binding);
        assert_eq!(denied.status, ObligationStatus::Fail);
    }
}
