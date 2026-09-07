//! RFC 0007 proof-kernel tranche 0.2: genuine dependency reference checker.
//!
//! The MNCS-native kernel lives in `library/core/proof_dep.mncs`; this module
//! is the independent second implementation for differential validation. It
//! encodes the same v2 calculus as `docs/rfc-0007-tranche-02-calculus.md`
//! with deliberate implementation diversity: direct recursive evaluation
//! with de Bruijn levels and fuel, versus the MNCS kernel's flat single-pass
//! tables over a `[Cell; 32]` buffer with explicit-substitution closures.
//! No code is shared between them.
//!
//! The differential contract is verdict-class agreement (PASS / FAIL /
//! UNKNOWN) plus exact agreement on assumption codes and probe outputs over
//! the shared corpus (`examples/execution/proof-dep-corpus.json`, produced by
//! `scripts/gen_proof_dep_corpus.py`). Agreement is evidence of consistency,
//! never a proof of checker correctness. If the two ever disagree, the
//! dispute is resolved in favour of NEITHER implementation: both are bugs
//! until the calculus document adjudicates.

use serde::{Deserialize, Serialize};

use crate::canonical::{canonical_json_value, sha256_hex};

pub const PROOF_DEP_KERNEL_ID: &str = "mncs:proof-kernel:0.2";
pub const PROOF_DEP_BUFFER_CAPACITY: usize = 32;
pub const PROOF_DEP_MAX_UNIVERSE: i64 = 3;
/// Recursion fuel for one checker entry: exhaustion is UNKNOWN, never PASS.
pub const PROOF_DEP_FUEL: u64 = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepTag {
    Unsupported,
    Universe,
    Var,
    Pi,
    Lam,
    App,
    Nat,
    Zero,
    Succ,
    Plus,
    NatElim,
    Eq,
    Refl,
    Hyp,
    Cong,
}

impl DepTag {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Universe" => Some(Self::Universe),
            "Var" => Some(Self::Var),
            "Pi" => Some(Self::Pi),
            "Lam" | "Lambda" => Some(Self::Lam),
            "App" | "Apply" => Some(Self::App),
            "Nat" => Some(Self::Nat),
            "Zero" => Some(Self::Zero),
            "Succ" => Some(Self::Succ),
            "Plus" => Some(Self::Plus),
            "NatElim" => Some(Self::NatElim),
            "Eq" => Some(Self::Eq),
            "Refl" => Some(Self::Refl),
            "Hyp" => Some(Self::Hyp),
            "Cong" => Some(Self::Cong),
            "Unsupported" => Some(Self::Unsupported),
            _ => None,
        }
    }

    /// Numeric codes shared with the MNCS kernel's `tag_code`.
    pub fn code(self) -> i64 {
        match self {
            Self::Universe => 1,
            Self::Var => 2,
            Self::Pi => 3,
            Self::Lam => 4,
            Self::App => 5,
            Self::Nat => 6,
            Self::Zero => 7,
            Self::Succ => 8,
            Self::Plus => 9,
            Self::NatElim => 10,
            Self::Eq => 11,
            Self::Refl => 12,
            Self::Hyp => 13,
            Self::Cong => 14,
            Self::Unsupported => 0,
        }
    }

    /// Argument slots that are child references (must order below the cell).
    pub fn ref_slots(self) -> &'static [usize] {
        match self {
            Self::Universe | Self::Var | Self::Nat | Self::Zero | Self::Unsupported => &[],
            Self::Pi | Self::Lam | Self::App => &[0, 1],
            Self::Succ | Self::Refl => &[0],
            Self::Plus => &[0, 1],
            Self::Hyp => &[1],
            Self::NatElim => &[0, 1, 2, 3],
            Self::Eq => &[0, 1, 2],
            Self::Cong => &[0, 1, 2],
        }
    }

    pub fn is_binder(self) -> bool {
        matches!(self, Self::Pi | Self::Lam | Self::Hyp)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepCell {
    pub tag: DepTag,
    pub args: [i64; 4],
}

impl DepCell {
    pub fn new(tag: DepTag, args: [i64; 4]) -> Self {
        Self { tag, args }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DepVerdict {
    Pass,
    Fail,
    Unknown,
}

impl DepVerdict {
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

    pub fn from_code(code: i64) -> Option<Self> {
        match code {
            0 => Some(Self::Pass),
            1 => Some(Self::Fail),
            2 => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// One entry of the shared differential corpus.
#[derive(Debug, Clone)]
pub struct DepCorpusCase {
    pub id: String,
    pub entry: DepEntry,
    pub cells: Vec<DepCell>,
    pub count: usize,
    pub first: usize,
    pub second: usize,
    pub expected: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepEntry {
    Check,
    Assumptions,
    Eval,
    Defeq,
}

fn corpus_integer(value: &serde_json::Value) -> Option<i64> {
    value.get("integer")?.get("value")?.as_i64()
}

fn corpus_byte(value: &serde_json::Value) -> Option<usize> {
    let byte = value.get("byte")?.get("value")?.as_u64()?;
    usize::try_from(byte).ok()
}

fn corpus_cell(value: &serde_json::Value) -> Option<DepCell> {
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
                tag = DepTag::from_name(short);
            }
            _ => return None,
        }
    }
    Some(DepCell::new(tag?, args))
}

/// Parse the tranche 0.2 differential corpus. Entry kind dispatches on the
/// request target function; `first`/`second` hold proof/prop, index, or
/// left/right depending on the entry.
pub fn parse_proof_dep_corpus(text: &str) -> Result<Vec<DepCorpusCase>, String> {
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
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "case has no id".to_owned())?
            .to_owned();
        let request = case
            .get("request")
            .ok_or_else(|| format!("{id}: no request"))?;
        let function = request
            .get("target")
            .and_then(|target| target.get("function"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("{id}: no target function"))?;
        let entry = match function {
            "check_proof_code" => DepEntry::Check,
            "assumptions_code" => DepEntry::Assumptions,
            "probe_eval_code" => DepEntry::Eval,
            "probe_defeq_code" => DepEntry::Defeq,
            other => return Err(format!("{id}: unknown entry {other}")),
        };
        let arguments = request
            .get("arguments")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{id}: no request arguments"))?;
        let values = arguments
            .first()
            .and_then(|first| first.get("sequence"))
            .and_then(|sequence| sequence.get("values"))
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{id}: no sequence values"))?;
        if values.len() != PROOF_DEP_BUFFER_CAPACITY {
            return Err(format!(
                "{id}: buffer is not {PROOF_DEP_BUFFER_CAPACITY} cells"
            ));
        }
        let mut cells = Vec::with_capacity(values.len());
        for value in values {
            cells.push(corpus_cell(value).ok_or_else(|| format!("{id}: bad cell"))?);
        }
        let count = corpus_byte(&arguments[1]).ok_or_else(|| format!("{id}: bad count"))?;
        let first = corpus_byte(&arguments[2]).ok_or_else(|| format!("{id}: bad index"))?;
        let second = if entry == DepEntry::Eval {
            0
        } else {
            corpus_byte(&arguments[3]).ok_or_else(|| format!("{id}: bad index"))?
        };
        let expected = case
            .get("expected")
            .and_then(serde_json::Value::as_array)
            .and_then(|expected| expected.first())
            .and_then(corpus_integer)
            .ok_or_else(|| format!("{id}: bad expectation"))?;
        parsed.push(DepCorpusCase {
            id,
            entry,
            cells,
            count,
            first,
            second,
            expected,
        });
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// Structure: topology, binders, resolution, sharing.
// Every violation is FAIL (malformed), never UNKNOWN.
// ---------------------------------------------------------------------------

pub struct DepBuffer {
    pub cells: Vec<DepCell>,
    pub count: usize,
}

impl DepBuffer {
    pub fn new(cells: Vec<DepCell>, count: usize) -> Self {
        Self { cells, count }
    }

    fn cell(&self, index: usize) -> Option<&DepCell> {
        self.cells.get(index)
    }

    /// Topological order plus immediate ranges.
    fn check_topo(&self) -> Result<(), DepVerdict> {
        if self.count == 0 || self.count > PROOF_DEP_BUFFER_CAPACITY {
            return Err(DepVerdict::Fail);
        }
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            let at = index as i64;
            for slot in cell.tag.ref_slots() {
                let child = cell.args[*slot];
                if child < 0 || child >= at {
                    return Err(DepVerdict::Fail);
                }
            }
            let immediate_ok = match cell.tag {
                DepTag::Universe => (0..=PROOF_DEP_MAX_UNIVERSE).contains(&cell.args[0]),
                DepTag::Var => (0..32).contains(&cell.args[0]),
                DepTag::Pi | DepTag::Lam | DepTag::Hyp => (0..32).contains(&level_of(cell)),
                _ => true,
            };
            if !immediate_ok {
                return Err(DepVerdict::Fail);
            }
        }
        Ok(())
    }

    /// Distinct active parents of every active cell.
    fn parents(&self) -> Vec<Vec<usize>> {
        let mut parents = vec![Vec::new(); self.count];
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            for slot in cell.tag.ref_slots() {
                let child = cell.args[*slot] as usize;
                if child < self.count && !parents[child].contains(&index) {
                    parents[child].push(index);
                }
            }
        }
        parents
    }

    /// Binder level to declaring cell. Duplicate levels are FAIL
    /// (Barendregt by construction, checked).
    fn binders(&self) -> Result<[Option<usize>; 32], DepVerdict> {
        let mut binders: [Option<usize>; 32] = [None; 32];
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            if cell.tag.is_binder() {
                let level = level_of(cell) as usize;
                if level >= 32 || binders[level].is_some() {
                    return Err(DepVerdict::Fail);
                }
                binders[level] = Some(index);
            }
        }
        Ok(binders)
    }

    /// Cells transitively mentioning a variable.
    fn mentions(&self) -> Vec<bool> {
        let mut mentions = vec![false; self.count];
        // Children always sit below their parents, so one upward pass
        // propagates mention flags to every ancestor.
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            let mut mentioned = cell.tag == DepTag::Var;
            for slot in cell.tag.ref_slots() {
                if mentions[cell.args[*slot] as usize] {
                    mentioned = true;
                }
            }
            mentions[index] = mentioned;
        }
        mentions
    }

    /// Nearest ancestor Hyp of `from`, if any.
    fn hyp_ancestor(&self, parents: &[Vec<usize>], from: usize) -> Option<usize> {
        let mut current = from;
        for _ in 0..PROOF_DEP_BUFFER_CAPACITY {
            let up = parents[current].first().copied()?;
            if self.cells[up].tag == DepTag::Hyp {
                return Some(up);
            }
            // Ambiguous or missing parents end the walk: resolution fails
            // closed elsewhere, and this query only refines Hyp errors.
            if parents[current].len() != 1 {
                return None;
            }
            current = up;
        }
        None
    }

    /// Resolve one variable occurrence to its binder cell. `Hyp` levels
    /// resolve buffer-globally; `Pi`/`Lam` levels resolve to the nearest
    /// ancestor binder, skipping domain edges (a domain sits outside its
    /// own binder's scope).
    fn resolve(
        &self,
        parents: &[Vec<usize>],
        binders: &[Option<usize>; 32],
        from: usize,
        level: i64,
    ) -> Result<usize, DepVerdict> {
        if !(0..32).contains(&level) {
            return Err(DepVerdict::Fail);
        }
        let declared = binders[level as usize].ok_or(DepVerdict::Fail)?;
        if self.cells[declared].tag == DepTag::Hyp {
            if self.hyp_ancestor(parents, from).is_some() {
                // Inside an assumption's type: assumption types must be
                // closed (dependent assumptions are a future boundary).
                return Err(DepVerdict::Fail);
            }
            return Ok(declared);
        }
        let mut current = from;
        let mut previous = from;
        for _ in 0..PROOF_DEP_BUFFER_CAPACITY {
            let ups = &parents[current];
            if ups.len() != 1 {
                return Err(DepVerdict::Fail);
            }
            let up = ups[0];
            let up_cell = &self.cells[up];
            let up_level = level_of(up_cell);
            let via_domain = matches!(up_cell.tag, DepTag::Pi | DepTag::Lam)
                && up_cell.args[0] as usize == previous;
            if matches!(up_cell.tag, DepTag::Pi | DepTag::Lam) && up_level == level && !via_domain {
                return Ok(up);
            }
            previous = current;
            current = up;
        }
        Err(DepVerdict::Fail)
    }

    /// Resolution targets for every variable (parallel to `cells`).
    pub fn targets(
        &self,
        parents: &[Vec<usize>],
        binders: &[Option<usize>; 32],
    ) -> Result<Vec<Option<usize>>, DepVerdict> {
        let mut targets = vec![None; self.count];
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            if cell.tag == DepTag::Var {
                targets[index] = Some(self.resolve(parents, binders, index, cell.args[0])?);
            }
        }
        Ok(targets)
    }

    /// Seed environment: every assumption maps to a rigid variable with
    /// its (closed) carrier type. Mirrors the MNCS static environment's
    /// buffer-global `Hyp` scope.
    pub fn hyp_seed(&self, fuel: &mut Fuel) -> Result<DepEnv, DepVerdict> {
        let mut env = DepEnv::default();
        for cell in self.cells.iter().take(self.count) {
            if cell.tag == DepTag::Hyp {
                let ty = self.eval(cell.args[1] as usize, &env, fuel)?;
                env = env.extended(cell.args[0], DepVal::Var(cell.args[0]), ty);
            }
        }
        Ok(env)
    }

    /// Reconstruct the lexical scope of one cell: walk up the single-parent
    /// chain, collecting enclosing `Pi`/`Lam` binders whose body edge leads
    /// here, then extend outermost-first with rigid variables. Domain edges
    /// contribute no frame (domains sit outside their binder's scope).
    /// Mirrors the MNCS static-environment pass.
    pub fn scope_env(
        &self,
        parents: &[Vec<usize>],
        index: usize,
        fuel: &mut Fuel,
    ) -> Result<DepEnv, DepVerdict> {
        let mut chain: Vec<(i64, usize)> = Vec::new();
        let mut current = index;
        loop {
            if chain.len() > self.count {
                return Err(DepVerdict::Fail);
            }
            let ups = &parents[current];
            if ups.len() != 1 {
                break;
            }
            let up = ups[0];
            let cell = &self.cells[up];
            if matches!(cell.tag, DepTag::Pi | DepTag::Lam) && cell.args[1] as usize == current {
                chain.push((level_of(cell), cell.args[0] as usize));
            }
            current = up;
        }
        let mut env = self.hyp_seed(fuel)?;
        for (level, domain) in chain.iter().rev() {
            let domain_val = self.eval(*domain, &env, fuel)?;
            env = env.extended(*level, DepVal::Var(*level), domain_val);
        }
        Ok(env)
    }

    /// Full structural check: topology, binder uniqueness, resolution, the
    /// open-cone tree rule (at most one distinct parent per open cell), and
    /// `Hyp`-as-root (declarations are not subterms).
    pub fn check_shape(&self) -> Result<Vec<Option<usize>>, DepVerdict> {
        self.check_topo()?;
        let parents = self.parents();
        let binders = self.binders()?;
        let mentions = self.mentions();
        let targets = self.targets(&parents, &binders)?;
        for (index, cell) in self.cells.iter().enumerate().take(self.count) {
            if cell.tag == DepTag::Var && targets[index].is_none() {
                return Err(DepVerdict::Fail);
            }
            if mentions[index] && parents[index].len() > 1 {
                return Err(DepVerdict::Fail);
            }
            if cell.tag == DepTag::Hyp && !parents[index].is_empty() {
                return Err(DepVerdict::Fail);
            }
        }
        Ok(targets)
    }
}

fn level_of(cell: &DepCell) -> i64 {
    match cell.tag {
        DepTag::Pi | DepTag::Lam => cell.args[2],
        DepTag::Hyp | DepTag::Var => cell.args[0],
        _ => -1,
    }
}

// ---------------------------------------------------------------------------
// Values, environments, evaluation, definitional equality.
// ---------------------------------------------------------------------------

/// A semantic value: terms and types share one domain. `Proof` is opaque
/// (proof objects compare as UNKNOWN, never by structure).
#[derive(Debug, Clone, PartialEq)]
pub enum DepVal {
    Univ(i64),
    Nat,
    Zero,
    Succ(Box<DepVal>),
    Plus(Box<DepVal>, Box<DepVal>),
    Pi(Box<DepVal>, DepClosure),
    Lam(Box<DepVal>, DepClosure),
    Eq(Box<DepVal>, Box<DepVal>, Box<DepVal>),
    Proof,
    Var(i64),
    Stuck,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClosureBody {
    /// A body cell to evaluate under the extended environment.
    Cell(usize),
    /// An already-evaluated codomain value (from `Lam` synthesis):
    /// application substitutes the rigid variable.
    Val(Box<DepVal>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepClosure {
    pub body: ClosureBody,
    pub env: DepEnv,
    pub level: i64,
}

impl DepClosure {
    pub fn cell(body: usize, env: DepEnv, level: i64) -> Self {
        Self {
            body: ClosureBody::Cell(body),
            env,
            level,
        }
    }

    pub fn value(value: DepVal, env: DepEnv, level: i64) -> Self {
        Self {
            body: ClosureBody::Val(Box::new(value)),
            env,
            level,
        }
    }
}

/// Capture-safe substitution of `argument` for rigid `level`. Shadowing is
/// unrepresentable (binder levels are unique), so substitution descends
/// everywhere except under a closure binding the same level.
fn subst_val(
    value: &DepVal,
    level: i64,
    argument: &DepVal,
    fuel: &mut Fuel,
) -> Result<DepVal, DepVerdict> {
    fuel.burn()?;
    match value {
        DepVal::Var(found) => {
            if *found == level {
                Ok(argument.clone())
            } else {
                Ok(value.clone())
            }
        }
        DepVal::Succ(inner) => Ok(DepVal::Succ(Box::new(subst_val(
            inner, level, argument, fuel,
        )?))),
        DepVal::Plus(left, right) => Ok(DepVal::Plus(
            Box::new(subst_val(left, level, argument, fuel)?),
            Box::new(subst_val(right, level, argument, fuel)?),
        )),
        DepVal::Pi(domain, closure) => Ok(DepVal::Pi(
            Box::new(subst_val(domain, level, argument, fuel)?),
            subst_closure(closure, level, argument, fuel)?,
        )),
        DepVal::Lam(domain, closure) => Ok(DepVal::Lam(
            Box::new(subst_val(domain, level, argument, fuel)?),
            subst_closure(closure, level, argument, fuel)?,
        )),
        DepVal::Eq(ty, left, right) => Ok(DepVal::Eq(
            Box::new(subst_val(ty, level, argument, fuel)?),
            Box::new(subst_val(left, level, argument, fuel)?),
            Box::new(subst_val(right, level, argument, fuel)?),
        )),
        DepVal::Univ(_) | DepVal::Nat | DepVal::Zero | DepVal::Proof | DepVal::Stuck => {
            Ok(value.clone())
        }
    }
}

fn subst_closure(
    closure: &DepClosure,
    level: i64,
    argument: &DepVal,
    fuel: &mut Fuel,
) -> Result<DepClosure, DepVerdict> {
    if closure.level == level {
        return Ok(closure.clone());
    }
    let mut env = DepEnv::default();
    for (entry, term, ty) in &closure.env.frames {
        env.frames.push((
            *entry,
            subst_val(term, level, argument, fuel)?,
            subst_val(ty, level, argument, fuel)?,
        ));
    }
    let body = match &closure.body {
        ClosureBody::Cell(index) => ClosureBody::Cell(*index),
        ClosureBody::Val(value) => {
            ClosureBody::Val(Box::new(subst_val(value, level, argument, fuel)?))
        }
    };
    Ok(DepClosure {
        body,
        env,
        level: closure.level,
    })
}

/// Levels map to (term value, type value). Rigid variables map to
/// themselves with their domain type.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DepEnv {
    pub frames: Vec<(i64, DepVal, DepVal)>,
}

impl DepEnv {
    pub fn extended(&self, level: i64, term: DepVal, ty: DepVal) -> Self {
        let mut frames = self.frames.clone();
        frames.push((level, term, ty));
        Self { frames }
    }

    pub fn lookup(&self, level: i64) -> Option<(DepVal, DepVal)> {
        self.frames
            .iter()
            .rev()
            .find(|(entry, _, _)| *entry == level)
            .map(|(_, term, ty)| (term.clone(), ty.clone()))
    }
}

pub struct Fuel(pub u64);

impl Fuel {
    pub fn burn(&mut self) -> Result<(), DepVerdict> {
        if self.0 == 0 {
            return Err(DepVerdict::Unknown);
        }
        self.0 -= 1;
        Ok(())
    }
}

fn plus_val(left: DepVal, right: DepVal, fuel: &mut Fuel) -> Result<DepVal, DepVerdict> {
    fuel.burn()?;
    match left {
        DepVal::Zero => Ok(right),
        DepVal::Succ(pred) => Ok(DepVal::Succ(Box::new(plus_val(*pred, right, fuel)?))),
        DepVal::Var(_) | DepVal::Stuck | DepVal::Plus(_, _) => {
            Ok(DepVal::Plus(Box::new(left), Box::new(right)))
        }
        _ => Err(DepVerdict::Unknown),
    }
}

/// Closed natural reading: `None` for open or ill-shaped values.
fn int_reading(value: &DepVal) -> Option<i64> {
    match value {
        DepVal::Zero => Some(0),
        DepVal::Succ(pred) => int_reading(pred)?.checked_add(1),
        DepVal::Plus(left, right) => int_reading(left)?.checked_add(int_reading(right)?),
        _ => None,
    }
}

fn apply_closure(
    buffer: &DepBuffer,
    closure: &DepClosure,
    argument: DepVal,
    arg_type: DepVal,
    fuel: &mut Fuel,
) -> Result<DepVal, DepVerdict> {
    match &closure.body {
        ClosureBody::Cell(body) => {
            let env = closure.env.extended(closure.level, argument, arg_type);
            buffer.eval(*body, &env, fuel)
        }
        ClosureBody::Val(value) => subst_val(value, closure.level, &argument, fuel),
    }
}

impl DepBuffer {
    pub fn eval(&self, index: usize, env: &DepEnv, fuel: &mut Fuel) -> Result<DepVal, DepVerdict> {
        fuel.burn()?;
        let cell = self.cell(index).ok_or(DepVerdict::Fail)?.clone();
        let arg = |slot: usize| cell.args[slot] as usize;
        match cell.tag {
            DepTag::Universe => Ok(DepVal::Univ(cell.args[0])),
            DepTag::Nat => Ok(DepVal::Nat),
            DepTag::Zero => Ok(DepVal::Zero),
            DepTag::Var => env
                .lookup(cell.args[0])
                .map(|(term, _)| term)
                .ok_or(DepVerdict::Fail),
            DepTag::Pi => {
                let domain = self.eval(arg(0), env, fuel)?;
                Ok(DepVal::Pi(
                    Box::new(domain),
                    DepClosure::cell(arg(1), env.clone(), cell.args[2]),
                ))
            }
            DepTag::Lam => {
                let domain = self.eval(arg(0), env, fuel)?;
                Ok(DepVal::Lam(
                    Box::new(domain),
                    DepClosure::cell(arg(1), env.clone(), cell.args[2]),
                ))
            }
            DepTag::App => {
                let function = self.eval(arg(0), env, fuel)?;
                let argument = self.eval(arg(1), env, fuel)?;
                match function {
                    DepVal::Lam(_, closure) => {
                        // Evaluation never reads environment types (only
                        // inference does), so the dummy is unobservable here.
                        apply_closure(self, &closure, argument, DepVal::Stuck, fuel)
                    }
                    DepVal::Var(_) | DepVal::Stuck => Ok(DepVal::Stuck),
                    _ => Err(DepVerdict::Unknown),
                }
            }
            DepTag::Succ => Ok(DepVal::Succ(Box::new(self.eval(arg(0), env, fuel)?))),
            DepTag::Plus => {
                let left = self.eval(arg(0), env, fuel)?;
                let right = self.eval(arg(1), env, fuel)?;
                plus_val(left, right, fuel)
            }
            DepTag::NatElim => self.eval_elim(&cell, env, fuel),
            DepTag::Eq => {
                let ty = self.eval(arg(0), env, fuel)?;
                let left = self.eval(arg(1), env, fuel)?;
                let right = self.eval(arg(2), env, fuel)?;
                Ok(DepVal::Eq(Box::new(ty), Box::new(left), Box::new(right)))
            }
            DepTag::Refl | DepTag::Cong => Ok(DepVal::Proof),
            DepTag::Hyp => Err(DepVerdict::Fail),
            DepTag::Unsupported => Err(DepVerdict::Unknown),
        }
    }

    fn eval_elim(
        &self,
        cell: &DepCell,
        env: &DepEnv,
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        let arg = |slot: usize| cell.args[slot] as usize;
        let motive = self.eval(arg(0), env, fuel)?;
        let target = self.eval(arg(3), env, fuel)?;
        // Evaluation needs only the motive's shape: genuine functions
        // unfold, neutrals stay stuck, anything else abstains.
        match motive {
            DepVal::Lam(_, _) => {}
            DepVal::Var(_) | DepVal::Stuck => return Ok(DepVal::Stuck),
            _ => return Err(DepVerdict::Unknown),
        }
        match target {
            DepVal::Zero => self.eval(arg(1), env, fuel),
            DepVal::Succ(pred) => {
                let step = self.eval(arg(2), env, fuel)?;
                self.elim_succ(step, arg(1), arg(2), (*pred).clone(), env, fuel)
            }
            DepVal::Var(_) | DepVal::Stuck | DepVal::Plus(_, _) => {
                if int_reading(&target).is_some() {
                    self.elim_closed(arg(1), arg(2), target, env, fuel)
                } else {
                    Ok(DepVal::Stuck)
                }
            }
            _ => Err(DepVerdict::Unknown),
        }
    }

    /// One iota unfolding for a successor target.
    fn elim_succ(
        &self,
        step: DepVal,
        zero: usize,
        step_cell: usize,
        predecessor: DepVal,
        env: &DepEnv,
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        fuel.burn()?;
        let rec = match predecessor.clone() {
            DepVal::Zero => self.eval(zero, env, fuel)?,
            DepVal::Succ(next) => {
                self.elim_succ(step.clone(), zero, step_cell, (*next).clone(), env, fuel)?
            }
            closed => {
                if int_reading(&closed).is_none() {
                    return Err(DepVerdict::Unknown);
                }
                self.elim_closed(zero, step_cell, closed, env, fuel)?
            }
        };
        let once = self.apply_value(step, predecessor, DepVal::Nat, fuel)?;
        self.apply_value(once, rec, DepVal::Nat, fuel)
    }

    /// Eliminator applied to a closed symbolic target: unfold the integer
    /// reading. Open targets stay stuck (handled by the caller).
    fn elim_closed(
        &self,
        zero: usize,
        step: usize,
        target: DepVal,
        env: &DepEnv,
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        let mut current = self.eval(zero, env, fuel)?;
        let steps = int_reading(&target).ok_or(DepVerdict::Unknown)?;
        let mut predecessor = DepVal::Zero;
        for _ in 0..steps {
            let step_val = self.eval(step, env, fuel)?;
            let once = self.apply_value(step_val, predecessor.clone(), DepVal::Nat, fuel)?;
            current = self.apply_value(once, current, DepVal::Nat, fuel)?;
            predecessor = DepVal::Succ(Box::new(predecessor));
        }
        Ok(current)
    }

    fn apply_value(
        &self,
        function: DepVal,
        argument: DepVal,
        arg_type: DepVal,
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        match function {
            DepVal::Lam(_, closure) => apply_closure(self, &closure, argument, arg_type, fuel),
            DepVal::Var(_) | DepVal::Stuck => Ok(DepVal::Stuck),
            _ => Err(DepVerdict::Unknown),
        }
    }

    /// The probe head encoding shared with `probe_eval_code`: singletons
    /// map to 2/3/1, pool values to their MNCS value tags.
    pub fn head_code(value: &DepVal) -> i64 {
        match value {
            DepVal::Nat => 2,
            DepVal::Zero => 3,
            DepVal::Univ(_) => 1,
            DepVal::Succ(_) => 4,
            DepVal::Plus(_, _) => 5,
            DepVal::Var(_) => 6,
            DepVal::Stuck => 7,
            DepVal::Pi(_, _) => 8,
            DepVal::Lam(_, _) => 9,
            DepVal::Eq(_, _, _) => 10,
            DepVal::Proof => 12,
        }
    }
}

/// Definitional equality over weak-head values. `depth` seeds rigid levels
/// at 32 and above (disjoint from every buffer level, hence capture-free).
/// Head mismatch is FAIL; stuck or opaque values are UNKNOWN.
pub fn defeq(
    buffer: &DepBuffer,
    left: &DepVal,
    right: &DepVal,
    depth: i64,
    fuel: &mut Fuel,
) -> Result<(), DepVerdict> {
    fuel.burn()?;
    // Identical values agree, except opaque proofs and stuck terms, which
    // abstain even against themselves.
    if left == right && !matches!(left, DepVal::Proof | DepVal::Stuck) {
        return Ok(());
    }
    match (left, right) {
        (DepVal::Univ(a), DepVal::Univ(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(DepVerdict::Fail)
            }
        }
        (DepVal::Nat, DepVal::Nat) => Ok(()),
        (DepVal::Var(a), DepVal::Var(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(DepVerdict::Fail)
            }
        }
        (DepVal::Succ(a), DepVal::Succ(b)) => defeq(buffer, a, b, depth, fuel),
        (DepVal::Plus(a, b), DepVal::Plus(c, d)) => {
            defeq(buffer, a, c, depth, fuel)?;
            defeq(buffer, b, d, depth, fuel)
        }
        (DepVal::Pi(a_dom, a_clo), DepVal::Pi(b_dom, b_clo)) => {
            defeq(buffer, a_dom, b_dom, depth, fuel)?;
            let rigid = DepVal::Var(32 + depth);
            let a_body = apply_closure(buffer, a_clo, rigid.clone(), (**a_dom).clone(), fuel)?;
            let b_body = apply_closure(buffer, b_clo, rigid, (**b_dom).clone(), fuel)?;
            defeq(buffer, &a_body, &b_body, depth + 1, fuel)
        }
        (DepVal::Lam(a_dom, a_clo), DepVal::Lam(b_dom, b_clo)) => {
            defeq(buffer, a_dom, b_dom, depth, fuel)?;
            let rigid = DepVal::Var(32 + depth);
            let a_body = apply_closure(buffer, a_clo, rigid.clone(), (**a_dom).clone(), fuel)?;
            let b_body = apply_closure(buffer, b_clo, rigid, (**b_dom).clone(), fuel)?;
            defeq(buffer, &a_body, &b_body, depth + 1, fuel)
        }
        (DepVal::Eq(a_t, a_l, a_r), DepVal::Eq(b_t, b_l, b_r)) => {
            defeq(buffer, a_t, b_t, depth, fuel)?;
            defeq(buffer, a_l, b_l, depth, fuel)?;
            defeq(buffer, a_r, b_r, depth, fuel)
        }
        _ => {
            // Closed numerals compare by integer reading even across
            // Succ/Plus shapes; everything else stuck or opaque abstains.
            match (int_reading(left), int_reading(right)) {
                (Some(a), Some(b)) => {
                    if a == b {
                        Ok(())
                    } else {
                        Err(DepVerdict::Fail)
                    }
                }
                _ => {
                    if matches!(left, DepVal::Stuck | DepVal::Proof)
                        || matches!(right, DepVal::Stuck | DepVal::Proof)
                    {
                        Err(DepVerdict::Unknown)
                    } else {
                        Err(DepVerdict::Fail)
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Inference: every cell synthesizes a (term, type) value pair.
// ---------------------------------------------------------------------------

/// A universe value destructured, a stuck type, or neither.
enum UnivShape {
    Level(i64),
    Stuck,
    Other,
}

fn univ_shape(value: &DepVal) -> UnivShape {
    match value {
        DepVal::Univ(level) => UnivShape::Level(*level),
        DepVal::Var(_) | DepVal::Stuck => UnivShape::Stuck,
        _ => UnivShape::Other,
    }
}

/// Types this kernel classifies: universes, `Nat`, `Pi` families, and
/// equalities. Rigid or stuck types abstain; anything else is ill-typed.
fn classifier(value: &DepVal) -> Result<(), DepVerdict> {
    match value {
        DepVal::Univ(_) | DepVal::Nat | DepVal::Pi(_, _) | DepVal::Eq(_, _, _) => Ok(()),
        DepVal::Var(_) | DepVal::Stuck => Err(DepVerdict::Unknown),
        _ => Err(DepVerdict::Fail),
    }
}

impl DepBuffer {
    /// Synthesize the (term, type) values of one cell.
    pub fn infer(
        &self,
        index: usize,
        env: &DepEnv,
        targets: &[Option<usize>],
        fuel: &mut Fuel,
    ) -> Result<(DepVal, DepVal), DepVerdict> {
        fuel.burn()?;
        let cell = self.cell(index).ok_or(DepVerdict::Fail)?.clone();
        let arg = |slot: usize| cell.args[slot] as usize;
        match cell.tag {
            DepTag::Universe => {
                let level = cell.args[0];
                if !(0..PROOF_DEP_MAX_UNIVERSE).contains(&level) {
                    return Err(DepVerdict::Fail);
                }
                Ok((DepVal::Univ(level), DepVal::Univ(level + 1)))
            }
            DepTag::Nat => Ok((DepVal::Nat, DepVal::Univ(0))),
            DepTag::Zero => Ok((DepVal::Zero, DepVal::Nat)),
            DepTag::Var => {
                let level = cell.args[0];
                if let Some((term, ty)) = env.lookup(level) {
                    return Ok((term, ty));
                }
                // Assumption use: the type elaborates under the empty
                // environment (assumption types are closed by structure).
                match targets.get(index).copied().flatten() {
                    Some(binder) if self.cells[binder].tag == DepTag::Hyp => {
                        let ty_cell = self.cells[binder].args[1] as usize;
                        let ty = self.eval(ty_cell, &DepEnv::default(), fuel)?;
                        Ok((DepVal::Var(level), ty))
                    }
                    _ => Err(DepVerdict::Fail),
                }
            }
            DepTag::Pi => {
                let (domain, domain_ty) = self.infer(arg(0), env, targets, fuel)?;
                let body_index = arg(1);
                let level = cell.args[2];
                let domain_level = match univ_shape(&domain_ty) {
                    UnivShape::Level(found) => found,
                    UnivShape::Stuck => return Err(DepVerdict::Unknown),
                    UnivShape::Other => return Err(DepVerdict::Fail),
                };
                let env_body = env.extended(level, DepVal::Var(level), domain.clone());
                let (_, body_ty) = self.infer(body_index, &env_body, targets, fuel)?;
                let body_level = match univ_shape(&body_ty) {
                    UnivShape::Level(found) => found,
                    UnivShape::Stuck => return Err(DepVerdict::Unknown),
                    UnivShape::Other => return Err(DepVerdict::Fail),
                };
                let joined = domain_level.max(body_level);
                if joined > PROOF_DEP_MAX_UNIVERSE {
                    return Err(DepVerdict::Fail);
                }
                let closure = DepClosure::cell(body_index, env.clone(), level);
                Ok((DepVal::Pi(Box::new(domain), closure), DepVal::Univ(joined)))
            }
            DepTag::Lam => {
                let (domain, domain_ty) = self.infer(arg(0), env, targets, fuel)?;
                let body_index = arg(1);
                let level = cell.args[2];
                match univ_shape(&domain_ty) {
                    UnivShape::Level(_) => {}
                    UnivShape::Stuck => return Err(DepVerdict::Unknown),
                    UnivShape::Other => return Err(DepVerdict::Fail),
                }
                let env_body = env.extended(level, DepVal::Var(level), domain.clone());
                let (_, body_ty) = self.infer(body_index, &env_body, targets, fuel)?;
                classifier(&body_ty)?;
                let term_closure = DepClosure::cell(body_index, env.clone(), level);
                let type_closure = DepClosure::value(body_ty, env_body, level);
                Ok((
                    DepVal::Lam(Box::new(domain.clone()), term_closure),
                    DepVal::Pi(Box::new(domain), type_closure),
                ))
            }
            DepTag::App => {
                let (function, function_ty) = self.infer(arg(0), env, targets, fuel)?;
                let (domain, codomain) = match function_ty {
                    DepVal::Pi(domain, codomain) => (domain, codomain),
                    DepVal::Var(_) | DepVal::Stuck => return Err(DepVerdict::Unknown),
                    _ => return Err(DepVerdict::Fail),
                };
                let (argument, argument_ty) = self.infer(arg(1), env, targets, fuel)?;
                defeq(self, &argument_ty, &domain, 0, fuel)?;
                let term = match function {
                    DepVal::Lam(_, closure) => {
                        apply_closure(self, &closure, argument.clone(), (*domain).clone(), fuel)?
                    }
                    DepVal::Var(_) | DepVal::Stuck => DepVal::Stuck,
                    _ => return Err(DepVerdict::Unknown),
                };
                let ty = apply_closure(self, &codomain, argument, (*domain).clone(), fuel)?;
                Ok((term, ty))
            }
            DepTag::Succ => {
                let (child, child_ty) = self.infer(arg(0), env, targets, fuel)?;
                defeq(self, &child_ty, &DepVal::Nat, 0, fuel)?;
                Ok((DepVal::Succ(Box::new(child)), DepVal::Nat))
            }
            DepTag::Plus => {
                let (left, left_ty) = self.infer(arg(0), env, targets, fuel)?;
                let (right, right_ty) = self.infer(arg(1), env, targets, fuel)?;
                defeq(self, &left_ty, &DepVal::Nat, 0, fuel)?;
                defeq(self, &right_ty, &DepVal::Nat, 0, fuel)?;
                Ok((plus_val(left, right, fuel)?, DepVal::Nat))
            }
            DepTag::NatElim => self.infer_elim(&cell, env, targets, fuel),
            DepTag::Eq => {
                let (ty, ty_ty) = self.infer(arg(0), env, targets, fuel)?;
                let universe = match univ_shape(&ty_ty) {
                    UnivShape::Level(found) => found,
                    UnivShape::Stuck => return Err(DepVerdict::Unknown),
                    UnivShape::Other => return Err(DepVerdict::Fail),
                };
                let (left, left_ty) = self.infer(arg(1), env, targets, fuel)?;
                let (right, right_ty) = self.infer(arg(2), env, targets, fuel)?;
                defeq(self, &left_ty, &ty, 0, fuel)?;
                defeq(self, &right_ty, &ty, 0, fuel)?;
                Ok((
                    DepVal::Eq(Box::new(ty), Box::new(left), Box::new(right)),
                    DepVal::Univ(universe),
                ))
            }
            DepTag::Refl => {
                let (witness, witness_ty) = self.infer(arg(0), env, targets, fuel)?;
                classifier(&witness_ty)?;
                Ok((
                    DepVal::Proof,
                    DepVal::Eq(
                        Box::new(witness_ty),
                        Box::new(witness.clone()),
                        Box::new(witness),
                    ),
                ))
            }
            // Declarations elaborate vacuously: a `Hyp` is not a term, but
            // its presence is well-formed (any *use* goes through the
            // assumption path in `Var`). Stuck dummies make misuse
            // abstain, matching the kernel's empty value slots.
            DepTag::Hyp => Ok((DepVal::Stuck, DepVal::Stuck)),
            DepTag::Cong => {
                let (_, proof_ty) = self.infer(arg(0), env, targets, fuel)?;
                let (ty, left, right) = match proof_ty {
                    DepVal::Eq(ty, left, right) => (*ty, *left, *right),
                    DepVal::Var(_) | DepVal::Stuck => return Err(DepVerdict::Unknown),
                    _ => return Err(DepVerdict::Fail),
                };
                defeq(self, &ty, &DepVal::Nat, 0, fuel)?;
                let succ_a = self.infer_succ_side(arg(1), env, targets, fuel)?;
                let succ_b = self.infer_succ_side(arg(2), env, targets, fuel)?;
                defeq(self, &succ_a, &left, 0, fuel)?;
                defeq(self, &succ_b, &right, 0, fuel)?;
                Ok((
                    DepVal::Proof,
                    DepVal::Eq(
                        Box::new(DepVal::Nat),
                        Box::new(DepVal::Succ(Box::new(left))),
                        Box::new(DepVal::Succ(Box::new(right))),
                    ),
                ))
            }
            DepTag::Unsupported => Err(DepVerdict::Unknown),
        }
    }

    /// The `Succ` sides of a `Cong`: the predecessor value of a
    /// constructor cell over `Nat`.
    fn infer_succ_side(
        &self,
        index: usize,
        env: &DepEnv,
        targets: &[Option<usize>],
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        let cell = self.cell(index).ok_or(DepVerdict::Fail)?.clone();
        if cell.tag != DepTag::Succ {
            return Err(DepVerdict::Fail);
        }
        let (term, ty) = self.infer(index, env, targets, fuel)?;
        defeq(self, &ty, &DepVal::Nat, 0, fuel)?;
        match term {
            DepVal::Succ(predecessor) => Ok(*predecessor),
            DepVal::Var(_) | DepVal::Stuck => Err(DepVerdict::Unknown),
            _ => Err(DepVerdict::Fail),
        }
    }

    /// Dependent `Nat` elimination with the step type checked by motive
    /// application under two rigid variables (never by rebuilding types).
    fn infer_elim(
        &self,
        cell: &DepCell,
        env: &DepEnv,
        targets: &[Option<usize>],
        fuel: &mut Fuel,
    ) -> Result<(DepVal, DepVal), DepVerdict> {
        let arg = |slot: usize| cell.args[slot] as usize;
        // The motive type (a `Pi`) governs the domain check and codomain
        // well-formedness; the motive *term* (a `Lam`) governs every family
        // application. The two live in different values: synthesis keeps the
        // codomain value on the `Pi` and the body closure on the `Lam`.
        let (motive_term, motive_ty) = self.infer(arg(0), env, targets, fuel)?;
        let (motive_domain, motive_codomain) = match motive_ty {
            DepVal::Pi(domain, codomain) => (domain, codomain),
            DepVal::Var(_) | DepVal::Stuck => return Err(DepVerdict::Unknown),
            _ => return Err(DepVerdict::Fail),
        };
        defeq(self, &motive_domain, &DepVal::Nat, 0, fuel)?;
        let codomain_open =
            apply_closure(self, &motive_codomain, DepVal::Var(32), DepVal::Nat, fuel)?;
        classifier(&codomain_open)?;
        let motive_fun = match motive_term {
            DepVal::Lam(_, closure) => Some(closure),
            DepVal::Var(_) | DepVal::Stuck => None,
            _ => return Err(DepVerdict::Unknown),
        };
        let family = |buffer: &Self, value: DepVal, ty: DepVal, fuel: &mut Fuel| match &motive_fun {
            Some(closure) => apply_closure(buffer, closure, value, ty, fuel),
            None => Ok(DepVal::Stuck),
        };
        let (zero_term, zero_ty) = self.infer(arg(1), env, targets, fuel)?;
        let motive_zero = family(self, DepVal::Zero, DepVal::Nat, fuel)?;
        defeq(self, &zero_ty, &motive_zero, 0, fuel)?;
        let (step_term, step_ty) = self.infer(arg(2), env, targets, fuel)?;
        let rigid_k = DepVal::Var(32);
        let step_inner = match step_ty {
            DepVal::Pi(domain, closure) => {
                defeq(self, &domain, &DepVal::Nat, 0, fuel)?;
                apply_closure(self, &closure, rigid_k.clone(), DepVal::Nat, fuel)?
            }
            DepVal::Var(_) | DepVal::Stuck => return Err(DepVerdict::Unknown),
            _ => return Err(DepVerdict::Fail),
        };
        let rigid_ih = DepVal::Var(33);
        let (ih_domain, ih_closure) = match step_inner {
            DepVal::Pi(domain, closure) => (domain, closure),
            DepVal::Var(_) | DepVal::Stuck => return Err(DepVerdict::Unknown),
            _ => return Err(DepVerdict::Fail),
        };
        let motive_k = family(self, rigid_k.clone(), DepVal::Nat, fuel)?;
        defeq(self, &ih_domain, &motive_k, 0, fuel)?;
        let got = apply_closure(self, &ih_closure, rigid_ih, motive_k, fuel)?;
        let want = family(
            self,
            DepVal::Succ(Box::new(rigid_k.clone())),
            DepVal::Nat,
            fuel,
        )?;
        defeq(self, &got, &want, 0, fuel)?;
        let (target_term, target_ty) = self.infer(arg(3), env, targets, fuel)?;
        defeq(self, &target_ty, &DepVal::Nat, 0, fuel)?;
        let ty = family(self, target_term.clone(), DepVal::Nat, fuel)?;
        let term = match target_term {
            DepVal::Zero => zero_term,
            DepVal::Succ(predecessor) => {
                let rec =
                    self.elim_rec(arg(1), arg(2), (*predecessor).clone(), env, targets, fuel)?;
                let once =
                    self.apply_value(step_term, (*predecessor).clone(), DepVal::Nat, fuel)?;
                let rec_ty = family(self, (*predecessor).clone(), DepVal::Nat, fuel)?;
                self.apply_value(once, rec, rec_ty, fuel)?
            }
            DepVal::Var(_) | DepVal::Stuck => DepVal::Stuck,
            closed => {
                if int_reading(&closed).is_some() {
                    self.elim_rec_closed(arg(1), arg(2), closed, env, targets, fuel)?
                } else {
                    return Err(DepVerdict::Unknown);
                }
            }
        };
        Ok((term, ty))
    }

    /// The eliminator's recursive call on a predecessor, with types from
    /// the motive family.
    fn elim_rec(
        &self,
        zero: usize,
        step: usize,
        predecessor: DepVal,
        env: &DepEnv,
        targets: &[Option<usize>],
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        fuel.burn()?;
        match predecessor {
            DepVal::Zero => Ok(self.infer(zero, env, targets, fuel)?.0),
            DepVal::Succ(next) => {
                let rec = self.elim_rec(zero, step, (*next).clone(), env, targets, fuel)?;
                let step_term = self.infer(step, env, targets, fuel)?.0;
                let once = self.apply_value(step_term, (*next).clone(), DepVal::Nat, fuel)?;
                self.apply_value(once, rec, DepVal::Nat, fuel)
            }
            closed => {
                if int_reading(&closed).is_none() {
                    return Err(DepVerdict::Unknown);
                }
                self.elim_rec_closed(zero, step, closed, env, targets, fuel)
            }
        }
    }

    fn elim_rec_closed(
        &self,
        zero: usize,
        step: usize,
        target: DepVal,
        env: &DepEnv,
        targets: &[Option<usize>],
        fuel: &mut Fuel,
    ) -> Result<DepVal, DepVerdict> {
        let mut current = self.infer(zero, env, targets, fuel)?.0;
        let steps = int_reading(&target).ok_or(DepVerdict::Unknown)?;
        let mut predecessor = DepVal::Zero;
        for _ in 0..steps {
            let step_term = self.infer(step, env, targets, fuel)?.0;
            let once = self.apply_value(step_term, predecessor.clone(), DepVal::Nat, fuel)?;
            current = self.apply_value(once, current, DepVal::Nat, fuel)?;
            predecessor = DepVal::Succ(Box::new(predecessor));
        }
        Ok(current)
    }
}

// ---------------------------------------------------------------------------
// Entries: check, assumptions, and the two probes.
// ---------------------------------------------------------------------------

fn valid_range(count: usize, indices: &[usize]) -> bool {
    count > 0 && count <= PROOF_DEP_BUFFER_CAPACITY && indices.iter().all(|index| *index < count)
}

fn is_hyp(cells: &[DepCell], index: usize) -> bool {
    cells
        .get(index)
        .map(|cell| cell.tag == DepTag::Hyp)
        .unwrap_or(false)
}

/// `check_proof_code`: 0 PASS, 1 FAIL, 2 UNKNOWN.
///
/// Mirrors the MNCS verdict join: every active cell is inferred and the
/// worst verdict dominates (FAIL > UNKNOWN > PASS), then the claim is
/// checked. Traversal order can pick which verdict surfaces when a buffer
/// mixes FAIL and UNKNOWN faults; the corpus pins the agreed class.
pub fn dep_check(cells: Vec<DepCell>, count: usize, proof: usize, prop: usize) -> i64 {
    if !valid_range(count, &[proof, prop]) || is_hyp(&cells, proof) || is_hyp(&cells, prop) {
        return DepVerdict::Fail.code();
    }
    let buffer = DepBuffer::new(cells, count);
    let targets = match buffer.check_shape() {
        Ok(targets) => targets,
        Err(verdict) => return verdict.code(),
    };
    let parents = buffer.parents();
    let mut fuel = Fuel(PROOF_DEP_FUEL);
    let mut joined = DepVerdict::Pass;
    let mut proof_ty = None;
    let mut prop_val = None;
    for index in 0..count {
        let env = match buffer.scope_env(&parents, index, &mut fuel) {
            Ok(env) => env,
            Err(verdict) => {
                joined = joined.dominate(verdict);
                continue;
            }
        };
        match buffer.infer(index, &env, &targets, &mut fuel) {
            Ok((term, ty)) => {
                if index == proof {
                    proof_ty = Some(ty);
                }
                if index == prop {
                    prop_val = Some(term);
                }
            }
            Err(verdict) => {
                joined = joined.dominate(verdict);
            }
        }
    }
    if joined != DepVerdict::Pass {
        return joined.code();
    }
    match (proof_ty, prop_val) {
        (Some(proof_ty), Some(prop_val)) => {
            match defeq(&buffer, &proof_ty, &prop_val, 0, &mut fuel) {
                Ok(()) => DepVerdict::Pass.code(),
                Err(verdict) => verdict.code(),
            }
        }
        _ => DepVerdict::Unknown.code(),
    }
}

/// `assumptions_code`: base-33 over buffer order, -1 unless PASS.
pub fn dep_assumptions(cells: Vec<DepCell>, count: usize, proof: usize, prop: usize) -> i64 {
    if !valid_range(count, &[proof, prop]) || is_hyp(&cells, proof) || is_hyp(&cells, prop) {
        return -1;
    }
    if dep_check(cells.clone(), count, proof, prop) != DepVerdict::Pass.code() {
        return -1;
    }
    let buffer = DepBuffer::new(cells, count);
    // The re-check cannot fail after a PASS, but a second failure still
    // reports -1 rather than an assumption set.
    let targets = buffer.check_shape().unwrap_or_else(|_| vec![None; count]);
    let mut cones = vec![false; count];
    let mut stack = vec![proof, prop];
    while let Some(next) = stack.pop() {
        if next >= count || cones[next] {
            continue;
        }
        cones[next] = true;
        let cell = &buffer.cells[next];
        for slot in cell.tag.ref_slots() {
            stack.push(cell.args[*slot] as usize);
        }
    }
    let mut code = 0i64;
    for (index, cell) in buffer.cells.iter().enumerate().take(count) {
        let mut digit = 0i64;
        if cell.tag == DepTag::Hyp {
            let used = (0..count).any(|var| {
                cones[var] && buffer.cells[var].tag == DepTag::Var && targets[var] == Some(index)
            });
            if used {
                digit = cell.args[0] + 1;
            }
        }
        code = code.saturating_mul(33).saturating_add(digit);
    }
    code
}

/// `probe_eval_code`: value head, 100 on abstention, 101 on malformation.
pub fn dep_probe_eval(cells: Vec<DepCell>, count: usize, index: usize) -> i64 {
    if !valid_range(count, &[index]) {
        return 101;
    }
    let buffer = DepBuffer::new(cells, count);
    if buffer.check_shape().is_err() {
        return 101;
    }
    let parents = buffer.parents();
    let mut fuel = Fuel(PROOF_DEP_FUEL);
    let env = buffer
        .scope_env(&parents, index, &mut fuel)
        .unwrap_or_default();
    match buffer.eval(index, &env, &mut fuel) {
        Ok(value) => DepBuffer::head_code(&value),
        Err(_) => 100,
    }
}

/// `probe_defeq_code`: 0/1/2 over term values, 3 on malformation.
pub fn dep_probe_defeq(cells: Vec<DepCell>, count: usize, left: usize, right: usize) -> i64 {
    if !valid_range(count, &[left, right]) {
        return 3;
    }
    let buffer = DepBuffer::new(cells, count);
    let targets = match buffer.check_shape() {
        Ok(targets) => targets,
        Err(_) => return 3,
    };
    let parents = buffer.parents();
    let mut fuel = Fuel(PROOF_DEP_FUEL);
    let left_env = buffer
        .scope_env(&parents, left, &mut fuel)
        .unwrap_or_default();
    let left_val = buffer
        .infer(left, &left_env, &targets, &mut fuel)
        .map(|pair| pair.0)
        .unwrap_or(DepVal::Stuck);
    let right_env = buffer
        .scope_env(&parents, right, &mut fuel)
        .unwrap_or_default();
    let right_val = buffer
        .infer(right, &right_env, &targets, &mut fuel)
        .map(|pair| pair.0)
        .unwrap_or(DepVal::Stuck);
    match defeq(&buffer, &left_val, &right_val, 0, &mut fuel) {
        Ok(()) => DepVerdict::Pass.code(),
        Err(verdict) => verdict.code(),
    }
}

/// A sealed tranche-0.2 proof artifact: the exact buffer, the obligation it
/// discharges, and the observed assumption code. The content identity covers
/// every byte, so the `Hyp` declarations inside the buffer are part of the
/// sealed identity: adding, removing, or changing an assumption changes the
/// identity and invalidates every binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepArtifact {
    pub schema_version: String,
    pub identity: String,
    pub kernel: String,
    pub obligation: String,
    pub assumption_code: i64,
    pub cells: Vec<DepCellSer>,
    pub count: usize,
    pub proof: usize,
    pub proposition: usize,
}

/// Serializable `DepCell` (tuple struct encoding keeps canonical JSON small).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepCellSer {
    pub tag: String,
    pub args: [i64; 4],
}

impl DepCellSer {
    pub fn name(tag: DepTag) -> &'static str {
        match tag {
            DepTag::Unsupported => "Unsupported",
            DepTag::Universe => "Universe",
            DepTag::Var => "Var",
            DepTag::Pi => "Pi",
            DepTag::Lam => "Lam",
            DepTag::App => "App",
            DepTag::Nat => "Nat",
            DepTag::Zero => "Zero",
            DepTag::Succ => "Succ",
            DepTag::Plus => "Plus",
            DepTag::NatElim => "NatElim",
            DepTag::Eq => "Eq",
            DepTag::Refl => "Refl",
            DepTag::Hyp => "Hyp",
            DepTag::Cong => "Cong",
        }
    }

    pub fn of(cell: &DepCell) -> Self {
        Self {
            tag: Self::name(cell.tag).to_owned(),
            args: cell.args,
        }
    }

    pub fn to_cell(&self) -> Option<DepCell> {
        DepTag::from_name(&self.tag).map(|tag| DepCell::new(tag, self.args))
    }
}

fn dep_seal(artifact: &DepArtifact) -> String {
    let mut material = artifact.clone();
    material.identity = String::new();
    let canonical = canonical_json_value(&material).expect("artifact is serializable");
    format!("mncs:proof-dep:{}", sha256_hex(canonical.as_bytes()))
}

impl DepArtifact {
    pub const SCHEMA_VERSION: &'static str = "0.2";

    pub fn new(
        obligation: impl Into<String>,
        cells: Vec<DepCell>,
        count: usize,
        proof: usize,
        proposition: usize,
    ) -> Self {
        let assumption_code = dep_assumptions(cells.clone(), count, proof, proposition);
        let mut artifact = Self {
            schema_version: Self::SCHEMA_VERSION.to_owned(),
            identity: String::new(),
            kernel: PROOF_DEP_KERNEL_ID.to_owned(),
            obligation: obligation.into(),
            assumption_code,
            cells: cells.iter().map(DepCellSer::of).collect(),
            count,
            proof,
            proposition,
        };
        artifact.identity = dep_seal(&artifact);
        artifact
    }

    pub fn identity_is_valid(&self) -> bool {
        if self.schema_version != Self::SCHEMA_VERSION || self.identity.is_empty() {
            return false;
        }
        self.identity == dep_seal(self)
    }

    pub fn buffer(&self) -> Option<DepBuffer> {
        let mut cells = Vec::with_capacity(self.cells.len());
        for cell in &self.cells {
            cells.push(cell.to_cell()?);
        }
        Some(DepBuffer::new(cells, self.count))
    }
}

/// A kernel verdict bound to the exact artifact bytes it was checked under.
/// Reuse requires an exact match on identity, kernel, obligation, and the
/// observed assumption code: any material change invalidates the binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepBinding {
    pub proof: String,
    pub kernel: String,
    pub obligation: String,
    pub assumption_code: i64,
}

impl DepBinding {
    /// Bind a sealed artifact. Returns `None` unless the independent
    /// reference checker returns PASS for the exact artifact bytes *and*
    /// the recorded assumption code reproduces, so a binding always
    /// carries a checked verdict over a checked assumption set.
    pub fn bind(artifact: &DepArtifact) -> Option<Self> {
        if !artifact.identity_is_valid() || artifact.kernel != PROOF_DEP_KERNEL_ID {
            return None;
        }
        let buffer = artifact.buffer()?;
        let cells: Vec<DepCell> = buffer.cells.clone();
        if dep_check(
            cells.clone(),
            artifact.count,
            artifact.proof,
            artifact.proposition,
        ) != DepVerdict::Pass.code()
        {
            return None;
        }
        if dep_assumptions(cells, artifact.count, artifact.proof, artifact.proposition)
            != artifact.assumption_code
        {
            return None;
        }
        Some(Self {
            proof: artifact.identity.clone(),
            kernel: artifact.kernel.clone(),
            obligation: artifact.obligation.clone(),
            assumption_code: artifact.assumption_code,
        })
    }

    pub fn reusable_if(
        &self,
        proof: &str,
        kernel: &str,
        obligation: &str,
        assumption_code: i64,
    ) -> bool {
        self.proof == proof
            && self.kernel == kernel
            && self.obligation == obligation
            && self.assumption_code == assumption_code
    }
}

/// Run one shared-corpus case through the reference checker.
pub fn run_dep_case(case: &DepCorpusCase) -> i64 {
    match case.entry {
        DepEntry::Check => dep_check(case.cells.clone(), case.count, case.first, case.second),
        DepEntry::Assumptions => {
            dep_assumptions(case.cells.clone(), case.count, case.first, case.second)
        }
        DepEntry::Eval => dep_probe_eval(case.cells.clone(), case.count, case.first),
        DepEntry::Defeq => dep_probe_defeq(case.cells.clone(), case.count, case.first, case.second),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(tag: DepTag, args: [i64; 4]) -> DepCell {
        DepCell::new(tag, args)
    }

    fn nat() -> DepCell {
        cell(DepTag::Nat, [0, 0, 0, 0])
    }

    fn zero() -> DepCell {
        cell(DepTag::Zero, [0, 0, 0, 0])
    }

    fn var(level: i64) -> DepCell {
        cell(DepTag::Var, [level, 0, 0, 0])
    }

    /// The flagship: Pi (n : Nat). Eq Nat (Plus n Zero) n by induction.
    /// Mirrors `flagship_plus_zero_right` in `scripts/gen_proof_dep_corpus.py`
    /// cell for cell; any drift between the two is a bug in one of them.
    fn flagship_cells() -> Vec<DepCell> {
        vec![
            nat(),                                 // 0
            zero(),                                // 1
            var(1),                                // 2 motive n (use A)
            cell(DepTag::Plus, [2, 1, 0, 0]),      // 3 Plus n Zero
            var(1),                                // 4 motive n (use B)
            cell(DepTag::Eq, [0, 3, 4, 0]),        // 5 motive body
            cell(DepTag::Lam, [0, 5, 1, 0]),       // 6 motive
            cell(DepTag::Refl, [1, 0, 0, 0]),      // 7 base
            var(2),                                // 8 k (Plus-A)
            cell(DepTag::Plus, [8, 1, 0, 0]),      // 9 Plus k Zero
            cell(DepTag::Succ, [9, 0, 0, 0]),      // 10 sA
            var(2),                                // 11 k (sB)
            cell(DepTag::Succ, [11, 0, 0, 0]),     // 12 sB
            var(2),                                // 13 k (Plus-B)
            cell(DepTag::Plus, [13, 1, 0, 0]),     // 14 Plus-B
            var(2),                                // 15 k (P-k side)
            cell(DepTag::Eq, [0, 14, 15, 0]),      // 16 P k
            var(3),                                // 17 ih
            cell(DepTag::Cong, [17, 10, 12, 0]),   // 18 step body
            cell(DepTag::Lam, [16, 18, 3, 0]),     // 19 inner step
            cell(DepTag::Lam, [0, 19, 2, 0]),      // 20 outer step
            var(0),                                // 21 target n
            cell(DepTag::NatElim, [6, 7, 20, 21]), // 22
            cell(DepTag::Lam, [0, 22, 0, 0]),      // 23 proof
            var(4),                                // 24 prop n (use A)
            cell(DepTag::Plus, [24, 1, 0, 0]),     // 25
            var(4),                                // 26 prop n (use B)
            cell(DepTag::Eq, [0, 25, 26, 0]),      // 27
            cell(DepTag::Pi, [0, 27, 4, 0]),       // 28 prop
        ]
    }

    #[test]
    fn reference_passes_flagship_plus_zero_right() {
        assert_eq!(dep_check(flagship_cells(), 29, 23, 28), 0);
    }

    #[test]
    fn reference_reports_empty_assumptions_for_flagship() {
        assert_eq!(dep_assumptions(flagship_cells(), 29, 23, 28), 0);
    }

    #[test]
    fn reference_passes_closed_refl() {
        let cells = vec![
            nat(),
            zero(),
            cell(DepTag::Eq, [0, 1, 1, 0]),
            cell(DepTag::Refl, [1, 0, 0, 0]),
        ];
        assert_eq!(dep_check(cells, 4, 3, 2), 0);
    }

    #[test]
    fn reference_evaluates_beta_identity() {
        let cells = vec![
            nat(),
            zero(),
            var(5),
            cell(DepTag::Lam, [0, 2, 5, 0]),
            cell(DepTag::App, [3, 1, 0, 0]),
        ];
        assert_eq!(dep_probe_eval(cells, 5, 4), 3);
    }

    #[test]
    fn reference_computes_plus_defeq() {
        let cells = vec![
            nat(),
            zero(),
            cell(DepTag::Succ, [1, 0, 0, 0]),
            cell(DepTag::Succ, [2, 0, 0, 0]),
            cell(DepTag::Plus, [2, 2, 0, 0]),
            cell(DepTag::Succ, [2, 0, 0, 0]),
        ];
        assert_eq!(dep_probe_defeq(cells, 6, 4, 5), 0);
    }

    #[test]
    fn reference_resolves_nested_lam() {
        let cells = vec![
            nat(),
            var(5),
            cell(DepTag::Lam, [0, 1, 6, 0]),
            cell(DepTag::Lam, [0, 2, 5, 0]),
            cell(DepTag::Pi, [0, 0, 7, 0]),
            cell(DepTag::Pi, [0, 4, 8, 0]),
        ];
        assert_eq!(dep_check(cells, 6, 3, 5), 0);
    }

    #[test]
    fn reference_checks_const_family_elim() {
        let cells = vec![
            nat(),
            zero(),
            cell(DepTag::Lam, [0, 0, 5, 0]),
            var(7),
            cell(DepTag::Lam, [0, 3, 7, 0]),
            cell(DepTag::Lam, [0, 4, 6, 0]),
            cell(DepTag::NatElim, [2, 1, 5, 1]),
        ];
        assert_eq!(dep_check(cells, 7, 6, 0), 0);
    }

    #[test]
    fn reference_checks_open_proof_and_counts_assumption() {
        let cells = vec![
            nat(),
            cell(DepTag::Hyp, [0, 0, 0, 0]),
            var(0),
            var(0),
            var(0),
            cell(DepTag::Refl, [2, 0, 0, 0]),
            cell(DepTag::Eq, [0, 3, 4, 0]),
        ];
        assert_eq!(dep_check(cells.clone(), 7, 5, 6), 0);
        assert_eq!(dep_assumptions(cells, 7, 5, 6), 33i64.pow(5));
    }

    #[test]
    fn reference_fails_undeclared_variable() {
        let cells = vec![nat(), var(7)];
        assert_eq!(dep_check(cells, 2, 1, 0), 1);
    }

    #[test]
    fn reference_fails_hyp_as_proof() {
        let cells = vec![nat(), cell(DepTag::Hyp, [0, 0, 0, 0])];
        assert_eq!(dep_check(cells, 2, 1, 0), 1);
    }

    #[test]
    fn reference_fails_embedded_hyp() {
        let cells = vec![
            nat(),
            cell(DepTag::Hyp, [0, 0, 0, 0]),
            cell(DepTag::Succ, [1, 0, 0, 0]),
        ];
        assert_eq!(dep_check(cells, 3, 2, 0), 1);
    }

    #[test]
    fn reference_fails_dependent_hyp() {
        let cells = vec![
            nat(),
            var(3),
            cell(DepTag::Pi, [0, 1, 4, 0]),
            cell(DepTag::Hyp, [3, 2, 0, 0]),
        ];
        assert_eq!(dep_check(cells, 4, 2, 2), 1);
    }

    #[test]
    fn binding_seals_flagship_and_rejects_drift() {
        let artifact = DepArtifact::new(
            "mncs:obligation:plus-zero-right",
            flagship_cells(),
            29,
            23,
            28,
        );
        assert!(artifact.identity_is_valid());
        assert_eq!(artifact.assumption_code, 0);
        let binding = DepBinding::bind(&artifact).expect("flagship binds");
        assert!(binding.reusable_if(
            &artifact.identity,
            PROOF_DEP_KERNEL_ID,
            "mncs:obligation:plus-zero-right",
            0,
        ));
        // Drifted obligation invalidates reuse.
        assert!(!binding.reusable_if(
            &artifact.identity,
            PROOF_DEP_KERNEL_ID,
            "mncs:obligation:something-else",
            0,
        ));
        // Drifted kernel version invalidates reuse.
        assert!(!binding.reusable_if(
            &artifact.identity,
            "mncs:proof-kernel:0.1",
            "mncs:obligation:plus-zero-right",
            0,
        ));
        // A tampered buffer reseals to a different identity and no longer
        // matches the binding.
        let mut tampered = artifact.clone();
        tampered.cells[21] = DepCellSer::of(&var(1));
        assert_ne!(
            dep_check(
                tampered.buffer().expect("buffer").cells,
                tampered.count,
                tampered.proof,
                tampered.proposition,
            ),
            0
        );
    }

    #[test]
    fn binding_records_open_assumptions() {
        let cells = vec![
            nat(),
            cell(DepTag::Hyp, [0, 0, 0, 0]),
            var(0),
            var(0),
            var(0),
            cell(DepTag::Refl, [2, 0, 0, 0]),
            cell(DepTag::Eq, [0, 3, 4, 0]),
        ];
        let artifact = DepArtifact::new("mncs:obligation:open-refl", cells, 7, 5, 6);
        assert!(artifact.identity_is_valid());
        assert_eq!(artifact.assumption_code, 33i64.pow(5));
        let binding = DepBinding::bind(&artifact).expect("open proof binds");
        assert!(binding.reusable_if(
            &artifact.identity,
            PROOF_DEP_KERNEL_ID,
            "mncs:obligation:open-refl",
            33i64.pow(5),
        ));
        // The same proof under a different assumption set is not reusable.
        assert!(!binding.reusable_if(
            &artifact.identity,
            PROOF_DEP_KERNEL_ID,
            "mncs:obligation:open-refl",
            0,
        ));
    }

    #[test]
    fn artifact_survives_file_round_trip_and_rebinds() {
        let artifact = DepArtifact::new(
            "mncs:obligation:plus-zero-right",
            flagship_cells(),
            29,
            23,
            28,
        );
        let json = serde_json::to_string(&artifact).expect("serialize");
        let back: DepArtifact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, artifact);
        assert!(back.identity_is_valid());
        assert!(DepBinding::bind(&back).is_some());
        // A corrupted byte in transit invalidates the seal, never the verdict:
        // flip one hex digit inside the sealed identity (stays valid JSON).
        let mut tampered_json = json.clone();
        let marker = "\"identity\":\"mncs:proof-dep:";
        let start = tampered_json.find(marker).expect("identity") + marker.len();
        let flip = if tampered_json[start..].starts_with('a') {
            'b'
        } else {
            'a'
        };
        tampered_json.replace_range(start..start + 1, &flip.to_string());
        let tampered: DepArtifact =
            serde_json::from_str(&tampered_json).expect("deserialize tampered");
        assert!(!tampered.identity_is_valid());
        assert!(DepBinding::bind(&tampered).is_none());
    }

    #[test]
    fn reference_agrees_with_checked_in_corpus_expectations() {
        let path = format!(
            "{}/../../examples/execution/proof-dep-corpus.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&path).expect("proof-dep corpus");
        let cases = parse_proof_dep_corpus(&text).expect("parse proof-dep corpus");
        assert!(!cases.is_empty());
        for case in &cases {
            assert_eq!(
                run_dep_case(case),
                case.expected,
                "reference diverges on corpus case {}",
                case.id
            );
        }
    }
}
