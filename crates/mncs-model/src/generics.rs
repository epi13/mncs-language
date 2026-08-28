use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::body::{
    BodyBlock, BodyOperation, BodyOperationKind, BodyType, FunctionBody, GenericArg,
    GenericParamKind, SequenceBound,
};
use crate::canonical::sha256_hex;
use crate::identity::{function_id, instantiation_id, specialization_id, SemanticId};
use crate::{Function, Program, Value};

pub fn specialize_program(program: &Program) -> Result<Program, Vec<crate::Diagnostic>> {
    use crate::Diagnostic;
    let mut diagnostics = Vec::new();

    // Collect generic functions by identity.
    let mut generic_by_id: BTreeMap<SemanticId, &Function> = BTreeMap::new();
    for func in &program.functions {
        if !func.generic_params.is_empty() {
            let fid = function_id(func.identity_namespace(&program.module), &func.name);
            generic_by_id.insert(fid, func);
        }
    }
    if generic_by_id.is_empty() {
        return Ok(program.clone());
    }

    #[derive(Debug, Clone)]
    struct InstKey {
        generic_id: SemanticId,
        args: Vec<GenericArg>,
        canonical: String,
        hash: String,
        key: String,
    }

    let mut specialization_by_key: BTreeMap<String, Function> = BTreeMap::new();
    let mut specialization_records: Vec<crate::GenericSpecializationRecord> = Vec::new();
    let mut queue: VecDeque<InstKey> = VecDeque::new();
    let mut in_progress: BTreeSet<String> = BTreeSet::new();
    let mut seen_keys: BTreeSet<String> = BTreeSet::new();

    // Initial population from concrete (non-generic) functions
    for func in &program.functions {
        if !func.generic_params.is_empty() {
            continue;
        }
        let Some(body) = &func.body else {
            continue;
        };
        for block in &body.blocks {
            for op in &block.operations {
                if let BodyOperationKind::Call {
                    function: callee_id,
                    generic_args,
                    ..
                } = &op.kind
                {
                    if generic_args.is_empty() {
                        continue;
                    }
                    if let Some(callee_fn) = generic_by_id.get(callee_id) {
                        if !generic_args.iter().all(|a| a.is_concrete()) {
                            diagnostics.push(Diagnostic {
                                code: "MNE224".to_owned(),
                                path: format!("{}.body", func.name),
                                message: format!("generic call to '{}' has non-concrete arguments outside a generic context", callee_fn.name),
                            });
                            continue;
                        }
                        let canonical = generic_args
                            .iter()
                            .map(|a| a.canonical_string())
                            .collect::<Vec<_>>()
                            .join("|");
                        let key = format!("{}|{}", callee_id.0, canonical);
                        if !specialization_by_key.contains_key(&key) && !seen_keys.contains(&key) {
                            let hash = sha256_hex(canonical.as_bytes());
                            queue.push_back(InstKey {
                                generic_id: callee_id.clone(),
                                args: generic_args.clone(),
                                canonical,
                                hash,
                                key: key.clone(),
                            });
                            seen_keys.insert(key);
                        }
                    }
                }
            }
        }
    }

    const MAX_SPECIALIZATIONS: usize = 128;
    let mut expansions = 0usize;

    while let Some(inst) = queue.pop_front() {
        if specialization_by_key.len() >= MAX_SPECIALIZATIONS {
            diagnostics.push(Diagnostic {
                code: "MNE227".to_owned(),
                path: format!("specialization:{}", inst.generic_id.0),
                message: "generic specialization limit exceeded; possible expanding recursive instantiation".to_owned(),
            });
            break;
        }
        if specialization_by_key.contains_key(&inst.key) {
            continue;
        }
        if in_progress.contains(&inst.key) {
            continue;
        }
        in_progress.insert(inst.key.clone());
        let generic_fn = match generic_by_id.get(&inst.generic_id) {
            Some(f) => *f,
            None => {
                diagnostics.push(Diagnostic {
                    code: "MNE131".to_owned(),
                    path: inst.generic_id.0.clone(),
                    message: "generic function not found for specialization".to_owned(),
                });
                in_progress.remove(&inst.key);
                continue;
            }
        };
        if inst.args.len() != generic_fn.generic_params.len() {
            diagnostics.push(Diagnostic {
                code: "MNE221".to_owned(),
                path: generic_fn.name.clone(),
                message: format!("generic argument count mismatch for '{}'", generic_fn.name),
            });
            in_progress.remove(&inst.key);
            continue;
        }
        let mut type_map: BTreeMap<String, BodyType> = BTreeMap::new();
        let mut value_concrete: BTreeMap<String, u32> = BTreeMap::new();
        for (param, arg) in generic_fn.generic_params.iter().zip(&inst.args) {
            match (&param.kind, arg) {
                (GenericParamKind::Type, GenericArg::Type { ty }) => {
                    type_map.insert(param.name.clone(), ty.clone());
                }
                (GenericParamKind::Nat, GenericArg::Value { value }) => {
                    value_concrete.insert(param.name.clone(), *value);
                }
                _ => {
                    diagnostics.push(Diagnostic {
                        code: "MNE222".to_owned(),
                        path: generic_fn.name.clone(),
                        message: "generic argument kind mismatch".to_owned(),
                    });
                }
            }
        }
        // Detect expanding
        for (param_name, ty) in &type_map {
            let sem = ty.semantic_name();
            if sem.contains(param_name) && sem.contains('[') {
                diagnostics.push(Diagnostic {
                    code: "MNE227".to_owned(),
                    path: generic_fn.name.clone(),
                    message: format!(
                        "expanding generic substitution for '{}' appears self-referential",
                        param_name
                    ),
                });
                in_progress.remove(&inst.key);
                continue;
            }
        }
        let orig_body = match &generic_fn.body {
            Some(b) => b,
            None => {
                diagnostics.push(Diagnostic {
                    code: "MNE001".to_owned(),
                    path: generic_fn.name.clone(),
                    message: "generic function has no body".to_owned(),
                });
                in_progress.remove(&inst.key);
                continue;
            }
        };
        let new_body =
            substitute_function_body(orig_body, &type_map, &value_concrete, &BTreeMap::new());

        if body_has_unresolved_generics(&new_body) {
            diagnostics.push(Diagnostic {
                code: "MNE226".to_owned(),
                path: generic_fn.name.clone(),
                message: "generic specialization still contains unresolved generic parameters after substitution".to_owned(),
            });
            in_progress.remove(&inst.key);
            continue;
        }

        let hash = inst.hash.clone();
        let new_name = format!("{}__spec_{}", generic_fn.name, &hash[0..8]);
        let home = generic_fn.home_module.clone();
        let inst_id = instantiation_id(&inst.generic_id, &hash);

        // For inputs/outputs, substitute
        let mut new_inputs = Vec::new();
        for val in &generic_fn.inputs {
            let base_ty = body_type_for_standalone(program, generic_fn, &val.value_type);
            let concrete_ty =
                substitute_body_type(base_ty, &type_map, &value_concrete, &BTreeMap::new());
            new_inputs.push(Value {
                name: val.name.clone(),
                value_type: concrete_ty.semantic_name(),
            });
        }
        let mut new_outputs = Vec::new();
        for val in &generic_fn.outputs {
            let base_ty = body_type_for_standalone(program, generic_fn, &val.value_type);
            let concrete_ty =
                substitute_body_type(base_ty, &type_map, &value_concrete, &BTreeMap::new());
            new_outputs.push(Value {
                name: val.name.clone(),
                value_type: concrete_ty.semantic_name(),
            });
        }

        let new_function = Function {
            name: new_name.clone(),
            home_module: home.clone(),
            generic_params: Vec::new(),
            inputs: new_inputs,
            outputs: new_outputs,
            contracts: generic_fn.contracts.clone(),
            effects: generic_fn.effects.clone(),
            capabilities: generic_fn.capabilities.clone(),
            assumptions: generic_fn.assumptions.clone(),
            evidence: generic_fn.evidence.clone(),
            failure: generic_fn.failure.clone(),
            body: Some(new_body),
        };

        let record = crate::GenericSpecializationRecord {
            generic_function: inst.generic_id.clone(),
            specialization_function: function_id(
                home.as_deref().unwrap_or(&program.module),
                &new_name,
            ),
            instantiation: inst_id.clone(),
            args: inst.args.clone(),
            canonical_args: inst.canonical.clone(),
        };
        specialization_records.push(record);
        specialization_by_key.insert(inst.key.clone(), new_function.clone());

        // Scan new_body for further generic calls
        for block in new_function.body.as_ref().unwrap().blocks.iter() {
            for op in &block.operations {
                if let BodyOperationKind::Call {
                    function: callee_id,
                    generic_args,
                    ..
                } = &op.kind
                {
                    if generic_args.is_empty() {
                        continue;
                    }
                    if generic_by_id.contains_key(callee_id) {
                        if !generic_args.iter().all(|a| a.is_concrete()) {
                            continue;
                        }
                        let canonical = generic_args
                            .iter()
                            .map(|a| a.canonical_string())
                            .collect::<Vec<_>>()
                            .join("|");
                        let key = format!("{}|{}", callee_id.0, canonical);
                        if !specialization_by_key.contains_key(&key)
                            && !in_progress.contains(&key)
                            && !seen_keys.contains(&key)
                        {
                            if expansions >= MAX_SPECIALIZATIONS {
                                diagnostics.push(Diagnostic {
                                    code: "MNE227".to_owned(),
                                    path: new_name.clone(),
                                    message: "specialization expansion limit exceeded".to_owned(),
                                });
                            } else {
                                let hash2 = sha256_hex(canonical.as_bytes());
                                queue.push_back(InstKey {
                                    generic_id: callee_id.clone(),
                                    args: generic_args.clone(),
                                    canonical,
                                    hash: hash2,
                                    key: key.clone(),
                                });
                                seen_keys.insert(key);
                                expansions += 1;
                            }
                        }
                    }
                }
            }
        }

        in_progress.remove(&inst.key);
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut new_program = program.clone();
    for (_key, func) in specialization_by_key {
        if !new_program
            .functions
            .iter()
            .any(|f| f.name == func.name && f.home_module == func.home_module)
        {
            new_program.functions.push(func);
        }
    }
    new_program.functions.sort_by(|a, b| {
        let a_ns = a.home_module.as_deref().unwrap_or(&new_program.module);
        let b_ns = b.home_module.as_deref().unwrap_or(&new_program.module);
        (a_ns, &a.name).cmp(&(b_ns, &b.name))
    });
    new_program.generic_specializations = specialization_records;

    // Rewrite call sites
    let mut inst_to_spec: BTreeMap<String, SemanticId> = BTreeMap::new();
    for record in &new_program.generic_specializations {
        let key = format!("{}|{}", record.generic_function.0, record.canonical_args);
        inst_to_spec.insert(key, record.specialization_function.clone());
    }
    // Precompute spec_id -> name map to avoid borrowing new_program mutably and immutably at same time
    let spec_id_to_name: BTreeMap<SemanticId, String> = new_program
        .functions
        .iter()
        .map(|f| {
            (
                function_id(f.identity_namespace(&new_program.module), &f.name),
                f.name.clone(),
            )
        })
        .collect();
    for func in &mut new_program.functions {
        if !func.generic_params.is_empty() {
            continue;
        }
        let Some(body) = func.body.as_mut() else {
            continue;
        };
        for block in &mut body.blocks {
            for op in &mut block.operations {
                if let BodyOperationKind::Call {
                    function: callee_id,
                    function_name,
                    generic_args,
                    instantiation,
                    specialization,
                    ..
                } = &mut op.kind
                {
                    if generic_args.is_empty() {
                        continue;
                    }
                    if !generic_args.iter().all(|a| a.is_concrete()) {
                        continue;
                    }
                    let canonical = generic_args
                        .iter()
                        .map(|a| a.canonical_string())
                        .collect::<Vec<_>>()
                        .join("|");
                    let generic_id_clone = callee_id.clone();
                    let key = format!("{}|{}", callee_id.0, canonical);
                    if let Some(spec_id) = inst_to_spec.get(&key).cloned() {
                        let spec_name = spec_id_to_name
                            .get(&spec_id)
                            .cloned()
                            .unwrap_or_else(|| "unknown".to_owned());
                        let hash = sha256_hex(canonical.as_bytes());
                        *callee_id = spec_id.clone();
                        *function_name = spec_name;
                        *instantiation = Some(instantiation_id(&generic_id_clone, &hash));
                        *specialization = Some(spec_id.clone());
                        *generic_args = Vec::new();
                    }
                }
            }
        }
    }

    // Validate no generic call remains in concrete functions
    for func in &new_program.functions {
        if !func.generic_params.is_empty() {
            continue;
        }
        if let Some(body) = &func.body {
            for block in &body.blocks {
                for op in &block.operations {
                    if let BodyOperationKind::Call { generic_args, .. } = &op.kind {
                        if !generic_args.is_empty() && generic_args.iter().any(|a| !a.is_concrete())
                        {
                            diagnostics.push(Diagnostic {
                                code: "MNE226".to_owned(),
                                path: func.name.clone(),
                                message: "concrete function still contains unresolved generic arguments after specialization".to_owned(),
                            });
                        }
                    }
                }
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(new_program)
}

fn substitute_function_body(
    body: &crate::body::FunctionBody,
    type_map: &BTreeMap<String, BodyType>,
    value_concrete: &BTreeMap<String, u32>,
    value_param_map: &BTreeMap<String, String>,
) -> crate::body::FunctionBody {
    let mut new_body = body.clone();
    new_body.generic_params = Vec::new();
    for param in &mut new_body.parameters {
        param.ty =
            substitute_body_type(param.ty.clone(), type_map, value_concrete, value_param_map);
    }
    for block in &mut new_body.blocks {
        for param in &mut block.parameters {
            param.ty =
                substitute_body_type(param.ty.clone(), type_map, value_concrete, value_param_map);
        }
        for op in &mut block.operations {
            for res in &mut op.results {
                res.ty =
                    substitute_body_type(res.ty.clone(), type_map, value_concrete, value_param_map);
            }
            match &mut op.kind {
                BodyOperationKind::SequenceConstruct { element_type, .. } => {
                    *element_type = Box::new(substitute_body_type(
                        (**element_type).clone(),
                        type_map,
                        value_concrete,
                        value_param_map,
                    ));
                }
                BodyOperationKind::SequenceProject { bound, .. } => {
                    *bound =
                        substitute_sequence_bound(bound.clone(), value_concrete, value_param_map);
                }
                BodyOperationKind::SequenceLength { bound } => {
                    *bound =
                        substitute_sequence_bound(bound.clone(), value_concrete, value_param_map);
                }
                BodyOperationKind::ViewConstruct {
                    source_bound,
                    view_bound,
                } => {
                    *source_bound = substitute_sequence_bound(
                        source_bound.clone(),
                        value_concrete,
                        value_param_map,
                    );
                    *view_bound = substitute_sequence_bound(
                        view_bound.clone(),
                        value_concrete,
                        value_param_map,
                    );
                }
                BodyOperationKind::SequenceReplace {
                    element_type,
                    bound,
                    ..
                } => {
                    *element_type = Box::new(substitute_body_type(
                        (**element_type).clone(),
                        type_map,
                        value_concrete,
                        value_param_map,
                    ));
                    *bound =
                        substitute_sequence_bound(bound.clone(), value_concrete, value_param_map);
                }
                BodyOperationKind::VectorConstruct { element_type, .. }
                | BodyOperationKind::VectorSplat { element_type, .. }
                | BodyOperationKind::VectorExtract { element_type, .. }
                | BodyOperationKind::VectorReplace { element_type, .. }
                | BodyOperationKind::VectorBinary { element_type, .. }
                | BodyOperationKind::VectorCompare { element_type, .. }
                | BodyOperationKind::VectorReduce { element_type, .. } => {
                    *element_type = Box::new(substitute_body_type(
                        (**element_type).clone(),
                        type_map,
                        value_concrete,
                        value_param_map,
                    ));
                }
                BodyOperationKind::Convert { from, to } => {
                    *from = substitute_body_type(
                        from.clone(),
                        type_map,
                        value_concrete,
                        value_param_map,
                    );
                    *to =
                        substitute_body_type(to.clone(), type_map, value_concrete, value_param_map);
                }
                BodyOperationKind::Select { operand_type } => {
                    *operand_type = Box::new(substitute_body_type(
                        (**operand_type).clone(),
                        type_map,
                        value_concrete,
                        value_param_map,
                    ));
                }
                BodyOperationKind::Call { generic_args, .. } => {
                    for arg in generic_args.iter_mut() {
                        match arg {
                            GenericArg::Type { ty } => {
                                *ty = substitute_body_type(
                                    ty.clone(),
                                    type_map,
                                    value_concrete,
                                    value_param_map,
                                );
                            }
                            GenericArg::Value { .. } => {}
                            GenericArg::ValueParam { name } => {
                                if let Some(v) = value_concrete.get(name) {
                                    *arg = GenericArg::Value { value: *v };
                                } else if let Some(caller) = value_param_map.get(name) {
                                    *arg = GenericArg::ValueParam {
                                        name: caller.clone(),
                                    };
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for iter in &mut new_body.bounded_iterations {
        iter.state_type = substitute_body_type(
            iter.state_type.clone(),
            type_map,
            value_concrete,
            value_param_map,
        );
        match &mut iter.domain {
            crate::body::IterationDomain::OverSequence { element_type } => {
                *element_type = Box::new(substitute_body_type(
                    (**element_type).clone(),
                    type_map,
                    value_concrete,
                    value_param_map,
                ));
            }
            _ => {}
        }
        if iter.bound == crate::body::MAX_SEQUENCE_BOUND {
            if let Some((_k, v)) = value_concrete.iter().next() {
                iter.bound = *v;
            }
        }
    }
    new_body
}

fn substitute_body_type(
    ty: BodyType,
    type_map: &BTreeMap<String, BodyType>,
    value_concrete: &BTreeMap<String, u32>,
    value_param_map: &BTreeMap<String, String>,
) -> BodyType {
    match ty {
        BodyType::GenericParam { name } => type_map
            .get(&name)
            .cloned()
            .unwrap_or(BodyType::GenericParam { name }),
        BodyType::Sequence { element, bound } => {
            let new_elem = Box::new(substitute_body_type(
                *element,
                type_map,
                value_concrete,
                value_param_map,
            ));
            let new_bound = substitute_sequence_bound(bound, value_concrete, value_param_map);
            BodyType::Sequence {
                element: new_elem,
                bound: new_bound,
            }
        }
        BodyType::Vector { element, lanes } => BodyType::Vector {
            element: Box::new(substitute_body_type(
                *element,
                type_map,
                value_concrete,
                value_param_map,
            )),
            lanes,
        },
        other => other,
    }
}

fn substitute_sequence_bound(
    bound: SequenceBound,
    value_concrete: &BTreeMap<String, u32>,
    value_param_map: &BTreeMap<String, String>,
) -> SequenceBound {
    match bound {
        SequenceBound::Exact(v) => SequenceBound::Exact(v),
        SequenceBound::UpTo(v) => SequenceBound::UpTo(v),
        SequenceBound::Param(n) => {
            if let Some(v) = value_concrete.get(&n) {
                SequenceBound::Exact(*v)
            } else if let Some(caller) = value_param_map.get(&n) {
                SequenceBound::Param(caller.clone())
            } else {
                SequenceBound::Param(n)
            }
        }
        SequenceBound::UpToParam(n) => {
            if let Some(v) = value_concrete.get(&n) {
                SequenceBound::UpTo(*v)
            } else if let Some(caller) = value_param_map.get(&n) {
                SequenceBound::UpToParam(caller.clone())
            } else {
                SequenceBound::UpToParam(n)
            }
        }
    }
}

fn body_has_unresolved_generics(body: &FunctionBody) -> bool {
    for param in &body.parameters {
        if matches!(param.ty, BodyType::GenericParam { .. }) || param.ty.is_generic() {
            return true;
        }
    }
    for block in &body.blocks {
        for param in &block.parameters {
            if matches!(param.ty, BodyType::GenericParam { .. }) || param.ty.is_generic() {
                return true;
            }
        }
        for op in &block.operations {
            for res in &op.results {
                if matches!(res.ty, BodyType::GenericParam { .. }) || res.ty.is_generic() {
                    return true;
                }
            }
            match &op.kind {
                BodyOperationKind::Call { generic_args, .. } => {
                    if generic_args.iter().any(|a| !a.is_concrete()) {
                        return true;
                    }
                }
                BodyOperationKind::SequenceProject { bound, .. }
                | BodyOperationKind::SequenceLength { bound }
                | BodyOperationKind::SequenceReplace { bound, .. } => {
                    if bound.is_generic() {
                        return true;
                    }
                }
                BodyOperationKind::ViewConstruct {
                    source_bound,
                    view_bound,
                } => {
                    if source_bound.is_generic() || view_bound.is_generic() {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

trait BodyTypeExt2 {
    fn is_generic(&self) -> bool;
}
impl BodyTypeExt2 for BodyType {
    fn is_generic(&self) -> bool {
        match self {
            BodyType::GenericParam { .. } => true,
            BodyType::Sequence { element, bound } => element.is_generic() || bound.is_generic(),
            BodyType::Vector { element, .. } => element.is_generic(),
            _ => false,
        }
    }
}

fn body_type_for_standalone(program: &Program, function: &Function, value_type: &str) -> BodyType {
    if let Some(gp) = function
        .generic_params
        .iter()
        .find(|p| p.name == value_type && p.kind == GenericParamKind::Type)
    {
        return BodyType::GenericParam {
            name: gp.name.clone(),
        };
    }
    if value_type.trim_start().starts_with('[') {
        let parsed = BodyType::from_semantic_name(value_type);
        if let BodyType::Sequence { element, bound } = parsed {
            let new_element = match *element {
                BodyType::Named(n)
                    if function
                        .generic_params
                        .iter()
                        .any(|p| p.name == n && p.kind == GenericParamKind::Type) =>
                {
                    Box::new(BodyType::GenericParam { name: n })
                }
                other => Box::new(other),
            };
            return BodyType::Sequence {
                element: new_element,
                bound,
            };
        }
    }
    BodyType::from_program(program, value_type)
}
