//! A bounded WebAssembly MVP encoder and interpreter.
//!
//! This is a research execution envelope for the supported integer/control-flow
//! subset. It is not a general WASM engine and does not import host functions.

use mncs_model::{ExecutionStatus, ExecutionValue, IntegerType};

pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6d];
pub const WASM_VERSION: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    I32,
    I64,
}

impl ValType {
    fn byte(self) -> u8 {
        match self {
            Self::I32 => 0x7f,
            Self::I64 => 0x7e,
        }
    }

    fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x7f => Some(Self::I32),
            0x7e => Some(Self::I64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instr {
    Unreachable,
    Nop,
    Loop,
    If,
    Else,
    End,
    Br(u32),
    Return,
    Call(u32),
    Drop,
    LocalGet(u32),
    LocalSet(u32),
    I32Const(i32),
    I64Const(i64),
    I32Eqz,
    I32Eq,
    I32Ne,
    I32LtS,
    I32LtU,
    I32GtS,
    I32GtU,
    I32LeS,
    I32LeU,
    I32GeS,
    I32GeU,
    I64Eq,
    I64Ne,
    I64LtS,
    I64LtU,
    I64GtS,
    I64GtU,
    I64LeS,
    I64LeU,
    I64GeS,
    I64GeU,
    I32Add,
    I32Sub,
    I32Mul,
    I32And,
    I32Or,
    I32Xor,
    I64Add,
    I64Sub,
    I64Mul,
    I64And,
    I64Or,
    I64Xor,
    I32WrapI64,
    I64ExtendI32S,
    I64ExtendI32U,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmFunction {
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    pub locals: Vec<ValType>,
    pub body: Vec<Instr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub functions: Vec<WasmFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmTrap {
    pub status: ExecutionStatus,
    pub reason: String,
}

pub fn encode_module(module: &WasmModule) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&WASM_MAGIC);
    bytes.extend_from_slice(&WASM_VERSION);
    let mut types = Vec::new();
    for function in &module.functions {
        let mut ty = vec![0x60];
        encode_vec(&mut ty, &function.params, |out, ty| out.push(ty.byte()));
        encode_vec(&mut ty, &function.results, |out, ty| out.push(ty.byte()));
        types.push(ty);
    }
    emit_section(&mut bytes, 1, |section| {
        encode_vec(section, &types, |out, ty| out.extend_from_slice(ty));
    });
    emit_section(&mut bytes, 3, |section| {
        encode_u32(section, module.functions.len() as u32);
        for index in 0..module.functions.len() {
            encode_u32(section, index as u32);
        }
    });
    emit_section(&mut bytes, 7, |section| {
        encode_u32(section, module.functions.len() as u32);
        for (index, function) in module.functions.iter().enumerate() {
            encode_name(section, &function.name);
            section.push(0x00);
            encode_u32(section, index as u32);
        }
    });
    emit_section(&mut bytes, 10, |section| {
        encode_u32(section, module.functions.len() as u32);
        for function in &module.functions {
            let mut body = Vec::new();
            encode_u32(&mut body, function.locals.len() as u32);
            for local in &function.locals {
                encode_u32(&mut body, 1);
                body.push(local.byte());
            }
            for instr in &function.body {
                encode_instr(&mut body, instr);
            }
            body.push(0x0b);
            encode_u32(section, body.len() as u32);
            section.extend_from_slice(&body);
        }
    });
    bytes
}

pub fn decode_module(bytes: &[u8]) -> Result<WasmModule, WasmTrap> {
    if bytes.len() < 8 || bytes[..4] != WASM_MAGIC || bytes[4..8] != WASM_VERSION {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend artifact is not a WASM MVP module",
        ));
    }
    let mut cursor = 8;
    let mut types = Vec::new();
    let mut func_types = Vec::new();
    let mut exports = Vec::new();
    let mut bodies = Vec::new();
    while cursor < bytes.len() {
        let id = bytes[cursor];
        cursor += 1;
        let (size, next) = read_u32(bytes, cursor)?;
        cursor = next;
        let end = cursor + size as usize;
        if end > bytes.len() {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM section is truncated",
            ));
        }
        let payload = &bytes[cursor..end];
        match id {
            0 => {}
            1 => types = decode_types(payload)?,
            3 => func_types = decode_func_types(payload)?,
            7 => exports = decode_exports(payload)?,
            10 => bodies = decode_bodies(payload)?,
            _ => {
                return Err(trap(
                    ExecutionStatus::Unsupported,
                    format!("unsupported WASM section {id}"),
                ))
            }
        }
        cursor = end;
    }
    if func_types.len() != bodies.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "WASM function and code sections disagree",
        ));
    }
    let mut functions = Vec::new();
    for (index, (type_index, (locals, body))) in func_types.into_iter().zip(bodies).enumerate() {
        let ty = types.get(type_index as usize).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM function type is missing",
            )
        })?;
        let name = exports
            .iter()
            .find(|(_, exported)| *exported == index as u32)
            .map(|(name, _)| name.clone())
            .ok_or_else(|| {
                trap(
                    ExecutionStatus::InvalidRequest,
                    "WASM function is not exported",
                )
            })?;
        functions.push(WasmFunction {
            name,
            params: ty.0.clone(),
            results: ty.1.clone(),
            locals,
            body,
        });
    }
    Ok(WasmModule { functions })
}

pub fn execute_function(
    module: &WasmModule,
    name: &str,
    arguments: &[ExecutionValue],
    step_budget: u64,
) -> Result<Vec<ExecutionValue>, WasmTrap> {
    let function_index = module
        .functions
        .iter()
        .position(|function| function.name == name)
        .ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "backend export does not exist",
            )
        })?;
    let function = &module.functions[function_index];
    if arguments.len() != function.params.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend argument count does not match the exported function",
        ));
    }
    let mut steps = 0_u64;
    let opcode_budget = step_budget.saturating_mul(32).max(1);
    let raw_arguments = arguments
        .iter()
        .zip(&function.params)
        .map(|(argument, ty)| value_to_local(argument, *ty))
        .collect::<Result<Vec<_>, _>>()?;
    let returned = execute_raw(
        module,
        function_index,
        raw_arguments,
        opcode_budget,
        &mut steps,
        0,
    )?;
    function
        .results
        .iter()
        .zip(returned)
        .map(|(ty, value)| local_to_value(value, *ty))
        .collect()
}

fn execute_raw(
    module: &WasmModule,
    function_index: usize,
    arguments: Vec<i64>,
    opcode_budget: u64,
    steps: &mut u64,
    depth: usize,
) -> Result<Vec<i64>, WasmTrap> {
    if depth > module.functions.len() {
        return Err(trap(
            ExecutionStatus::RuntimeFailure,
            "backend call depth exceeded the acyclic module bound",
        ));
    }
    let function = module.functions.get(function_index).ok_or_else(|| {
        trap(
            ExecutionStatus::InvalidRequest,
            "backend call target index is out of range",
        )
    })?;
    if arguments.len() != function.params.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend call argument count does not match the callee",
        ));
    }
    let mut locals = vec![0_i64; function.params.len() + function.locals.len()];
    locals[..arguments.len()].copy_from_slice(&arguments);
    let mut stack = Vec::new();
    let mut labels = Vec::new();
    let mut ip = 0usize;
    while ip < function.body.len() {
        *steps = steps.saturating_add(1);
        if *steps > opcode_budget {
            return Err(trap(
                ExecutionStatus::BudgetExhausted,
                "backend execution step budget exhausted",
            ));
        }
        match &function.body[ip] {
            Instr::Unreachable => {
                return Err(trap(
                    ExecutionStatus::RuntimeFailure,
                    "backend trap: unreachable",
                ))
            }
            Instr::Nop => {}
            Instr::Loop => labels.push(Label {
                kind: LabelKind::Loop,
                start: ip + 1,
                stack_height: stack.len(),
            }),
            Instr::If => {
                let cond = pop_i32(&mut stack)?;
                labels.push(Label {
                    kind: LabelKind::If,
                    start: ip + 1,
                    stack_height: stack.len(),
                });
                if cond == 0 {
                    ip = skip_to_else_or_end(&function.body, ip)?;
                    continue;
                }
            }
            Instr::Else => {
                ip = skip_to_end(&function.body, ip)?;
                continue;
            }
            Instr::End => {
                labels.pop();
            }
            Instr::Br(depth) => {
                ip = branch(&function.body, &mut labels, &mut stack, *depth)?;
                continue;
            }
            Instr::Return => break,
            Instr::Call(callee_index) => {
                let callee = module
                    .functions
                    .get(*callee_index as usize)
                    .ok_or_else(|| {
                        trap(
                            ExecutionStatus::InvalidRequest,
                            "backend call target index is out of range",
                        )
                    })?;
                if stack.len() < callee.params.len() {
                    return Err(trap(
                        ExecutionStatus::InvalidRequest,
                        "backend call operand stack underflow",
                    ));
                }
                let arguments = stack.split_off(stack.len() - callee.params.len());
                let returned = execute_raw(
                    module,
                    *callee_index as usize,
                    arguments,
                    opcode_budget,
                    steps,
                    depth + 1,
                )?;
                stack.extend(returned);
            }
            Instr::Drop => {
                pop(&mut stack)?;
            }
            Instr::LocalGet(index) => {
                let value = *locals.get(*index as usize).ok_or_else(|| {
                    trap(
                        ExecutionStatus::InvalidRequest,
                        "backend local.get is out of range",
                    )
                })?;
                stack.push(value);
            }
            Instr::LocalSet(index) => {
                let value = pop(&mut stack)?;
                *locals.get_mut(*index as usize).ok_or_else(|| {
                    trap(
                        ExecutionStatus::InvalidRequest,
                        "backend local.set is out of range",
                    )
                })? = value;
            }
            Instr::I32Const(value) => stack.push(i64::from(*value)),
            Instr::I64Const(value) => stack.push(*value),
            Instr::I32Eqz => {
                let value = pop_i32(&mut stack)?;
                stack.push(i64::from(value == 0));
            }
            Instr::I32Eq => rel_i32(&mut stack, |left, right| left == right)?,
            Instr::I32Ne => rel_i32(&mut stack, |left, right| left != right)?,
            Instr::I32LtS => rel_i32(&mut stack, |left, right| left < right)?,
            Instr::I32LtU => rel_u32(&mut stack, |left, right| left < right)?,
            Instr::I32GtS => rel_i32(&mut stack, |left, right| left > right)?,
            Instr::I32GtU => rel_u32(&mut stack, |left, right| left > right)?,
            Instr::I32LeS => rel_i32(&mut stack, |left, right| left <= right)?,
            Instr::I32LeU => rel_u32(&mut stack, |left, right| left <= right)?,
            Instr::I32GeS => rel_i32(&mut stack, |left, right| left >= right)?,
            Instr::I32GeU => rel_u32(&mut stack, |left, right| left >= right)?,
            Instr::I64Eq => rel_i64(&mut stack, |left, right| left == right)?,
            Instr::I64Ne => rel_i64(&mut stack, |left, right| left != right)?,
            Instr::I64LtS => rel_i64(&mut stack, |left, right| left < right)?,
            Instr::I64LtU => rel_u64(&mut stack, |left, right| left < right)?,
            Instr::I64GtS => rel_i64(&mut stack, |left, right| left > right)?,
            Instr::I64GtU => rel_u64(&mut stack, |left, right| left > right)?,
            Instr::I64LeS => rel_i64(&mut stack, |left, right| left <= right)?,
            Instr::I64LeU => rel_u64(&mut stack, |left, right| left <= right)?,
            Instr::I64GeS => rel_i64(&mut stack, |left, right| left >= right)?,
            Instr::I64GeU => rel_u64(&mut stack, |left, right| left >= right)?,
            Instr::I32Add => bin_i32(&mut stack, |left, right| left.wrapping_add(right))?,
            Instr::I32Sub => bin_i32(&mut stack, |left, right| left.wrapping_sub(right))?,
            Instr::I32Mul => bin_i32(&mut stack, |left, right| left.wrapping_mul(right))?,
            Instr::I32And => bin_i32(&mut stack, |left, right| left & right)?,
            Instr::I32Or => bin_i32(&mut stack, |left, right| left | right)?,
            Instr::I32Xor => bin_i32(&mut stack, |left, right| left ^ right)?,
            Instr::I64Add => bin_i64(&mut stack, |left, right| left.wrapping_add(right))?,
            Instr::I64Sub => bin_i64(&mut stack, |left, right| left.wrapping_sub(right))?,
            Instr::I64Mul => bin_i64(&mut stack, |left, right| left.wrapping_mul(right))?,
            Instr::I64And => bin_i64(&mut stack, |left, right| left & right)?,
            Instr::I64Or => bin_i64(&mut stack, |left, right| left | right)?,
            Instr::I64Xor => bin_i64(&mut stack, |left, right| left ^ right)?,
            Instr::I32WrapI64 => {
                let value = pop(&mut stack)?;
                stack.push(i64::from(value as i32));
            }
            Instr::I64ExtendI32S => {
                let value = pop_i32(&mut stack)?;
                stack.push(i64::from(value));
            }
            Instr::I64ExtendI32U => {
                let value = pop_i32(&mut stack)? as u32;
                stack.push(i64::from(value));
            }
        }
        ip += 1;
    }
    if function.results.len() != stack.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend function did not leave the declared result count",
        ));
    }
    Ok(stack)
}

pub fn val_type_for(ty: IntegerType) -> Option<ValType> {
    match ty.bits {
        1..=32 => Some(ValType::I32),
        64 => Some(ValType::I64),
        _ => None,
    }
}

fn trap(status: ExecutionStatus, reason: impl Into<String>) -> WasmTrap {
    WasmTrap {
        status,
        reason: reason.into(),
    }
}

#[derive(Clone, Copy)]
enum LabelKind {
    Loop,
    If,
}

struct Label {
    kind: LabelKind,
    start: usize,
    stack_height: usize,
}

fn branch(
    body: &[Instr],
    labels: &mut Vec<Label>,
    stack: &mut Vec<i64>,
    depth: u32,
) -> Result<usize, WasmTrap> {
    if depth as usize >= labels.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend branch depth is out of range",
        ));
    }
    labels.truncate(labels.len() - depth as usize);
    let label = labels.pop().ok_or_else(|| {
        trap(
            ExecutionStatus::InvalidRequest,
            "backend branch has no label",
        )
    })?;
    stack.truncate(label.stack_height);
    match label.kind {
        LabelKind::Loop => {
            labels.push(label);
            Ok(labels.last().expect("loop label restored").start)
        }
        LabelKind::If => skip_to_end(body, label.start.saturating_sub(1)),
    }
}

fn skip_to_else_or_end(body: &[Instr], start: usize) -> Result<usize, WasmTrap> {
    let mut depth = 0usize;
    let mut ip = start + 1;
    while ip < body.len() {
        match body[ip] {
            Instr::Loop | Instr::If => depth += 1,
            Instr::Else if depth == 0 => return Ok(ip + 1),
            Instr::End if depth == 0 => return Ok(ip),
            Instr::End => depth -= 1,
            _ => {}
        }
        ip += 1;
    }
    Err(trap(
        ExecutionStatus::InvalidRequest,
        "WASM if is missing else/end",
    ))
}

fn skip_to_end(body: &[Instr], start: usize) -> Result<usize, WasmTrap> {
    let mut depth = 0usize;
    let mut ip = start + 1;
    while ip < body.len() {
        match body[ip] {
            Instr::Loop | Instr::If => depth += 1,
            Instr::End if depth == 0 => return Ok(ip),
            Instr::End => depth -= 1,
            _ => {}
        }
        ip += 1;
    }
    Err(trap(
        ExecutionStatus::InvalidRequest,
        "WASM structured instruction is missing end",
    ))
}

fn pop(stack: &mut Vec<i64>) -> Result<i64, WasmTrap> {
    stack.pop().ok_or_else(|| {
        trap(
            ExecutionStatus::InvalidRequest,
            "backend operand stack underflow",
        )
    })
}

fn pop_i32(stack: &mut Vec<i64>) -> Result<i32, WasmTrap> {
    Ok(pop(stack)? as i32)
}

fn bin_i32(stack: &mut Vec<i64>, op: impl Fn(i32, i32) -> i32) -> Result<(), WasmTrap> {
    let right = pop_i32(stack)?;
    let left = pop_i32(stack)?;
    stack.push(i64::from(op(left, right)));
    Ok(())
}

fn bin_i64(stack: &mut Vec<i64>, op: impl Fn(i64, i64) -> i64) -> Result<(), WasmTrap> {
    let right = pop(stack)?;
    let left = pop(stack)?;
    stack.push(op(left, right));
    Ok(())
}

fn rel_i32(stack: &mut Vec<i64>, op: impl Fn(i32, i32) -> bool) -> Result<(), WasmTrap> {
    let right = pop_i32(stack)?;
    let left = pop_i32(stack)?;
    stack.push(i64::from(op(left, right)));
    Ok(())
}

fn rel_u32(stack: &mut Vec<i64>, op: impl Fn(u32, u32) -> bool) -> Result<(), WasmTrap> {
    let right = pop_i32(stack)? as u32;
    let left = pop_i32(stack)? as u32;
    stack.push(i64::from(op(left, right)));
    Ok(())
}

fn rel_i64(stack: &mut Vec<i64>, op: impl Fn(i64, i64) -> bool) -> Result<(), WasmTrap> {
    let right = pop(stack)?;
    let left = pop(stack)?;
    stack.push(i64::from(op(left, right)));
    Ok(())
}

fn rel_u64(stack: &mut Vec<i64>, op: impl Fn(u64, u64) -> bool) -> Result<(), WasmTrap> {
    let right = pop(stack)? as u64;
    let left = pop(stack)? as u64;
    stack.push(i64::from(op(left, right)));
    Ok(())
}

fn value_to_local(value: &ExecutionValue, ty: ValType) -> Result<i64, WasmTrap> {
    match (value, ty) {
        (ExecutionValue::Integer { value, .. }, ValType::I32) => Ok(*value as i32 as i64),
        (ExecutionValue::Integer { value, .. }, ValType::I64) => Ok(*value as i64),
        (ExecutionValue::Boolean { value }, ValType::I32) => Ok(i64::from(*value)),
        (ExecutionValue::Finite { discriminant, .. }, ValType::I32) => Ok(i64::from(*discriminant)),
        _ => Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend argument does not match the WASM parameter type",
        )),
    }
}

fn local_to_value(value: i64, ty: ValType) -> Result<ExecutionValue, WasmTrap> {
    match ty {
        ValType::I32 => Ok(ExecutionValue::Integer {
            value: i128::from(value as i32),
            ty: IntegerType {
                bits: 32,
                signed: true,
            },
        }),
        ValType::I64 => Ok(ExecutionValue::Integer {
            value: i128::from(value),
            ty: IntegerType {
                bits: 64,
                signed: true,
            },
        }),
    }
}

fn emit_section(bytes: &mut Vec<u8>, id: u8, write: impl FnOnce(&mut Vec<u8>)) {
    let mut payload = Vec::new();
    write(&mut payload);
    bytes.push(id);
    encode_u32(bytes, payload.len() as u32);
    bytes.extend_from_slice(&payload);
}

fn encode_vec<T>(out: &mut Vec<u8>, items: &[T], write: impl Fn(&mut Vec<u8>, &T)) {
    encode_u32(out, items.len() as u32);
    for item in items {
        write(out, item);
    }
}

fn encode_name(out: &mut Vec<u8>, name: &str) {
    encode_u32(out, name.len() as u32);
    out.extend_from_slice(name.as_bytes());
}

fn encode_u32(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_i32(out: &mut Vec<u8>, mut value: i32) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn encode_i64(out: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

fn encode_instr(out: &mut Vec<u8>, instr: &Instr) {
    match instr {
        Instr::Unreachable => out.push(0x00),
        Instr::Nop => out.push(0x01),
        Instr::Loop => {
            out.push(0x03);
            out.push(0x40);
        }
        Instr::If => {
            out.push(0x04);
            out.push(0x40);
        }
        Instr::Else => out.push(0x05),
        Instr::End => out.push(0x0b),
        Instr::Br(depth) => {
            out.push(0x0c);
            encode_u32(out, *depth);
        }
        Instr::Return => out.push(0x0f),
        Instr::Call(index) => {
            out.push(0x10);
            encode_u32(out, *index);
        }
        Instr::Drop => out.push(0x1a),
        Instr::LocalGet(index) => {
            out.push(0x20);
            encode_u32(out, *index);
        }
        Instr::LocalSet(index) => {
            out.push(0x21);
            encode_u32(out, *index);
        }
        Instr::I32Const(value) => {
            out.push(0x41);
            encode_i32(out, *value);
        }
        Instr::I64Const(value) => {
            out.push(0x42);
            encode_i64(out, *value);
        }
        Instr::I32Eqz => out.push(0x45),
        Instr::I32Eq => out.push(0x46),
        Instr::I32Ne => out.push(0x47),
        Instr::I32LtS => out.push(0x48),
        Instr::I32LtU => out.push(0x49),
        Instr::I32GtS => out.push(0x4a),
        Instr::I32GtU => out.push(0x4b),
        Instr::I32LeS => out.push(0x4c),
        Instr::I32LeU => out.push(0x4d),
        Instr::I32GeS => out.push(0x4e),
        Instr::I32GeU => out.push(0x4f),
        Instr::I64Eq => out.push(0x51),
        Instr::I64Ne => out.push(0x52),
        Instr::I64LtS => out.push(0x53),
        Instr::I64LtU => out.push(0x54),
        Instr::I64GtS => out.push(0x55),
        Instr::I64GtU => out.push(0x56),
        Instr::I64LeS => out.push(0x57),
        Instr::I64LeU => out.push(0x58),
        Instr::I64GeS => out.push(0x59),
        Instr::I64GeU => out.push(0x5a),
        Instr::I32Add => out.push(0x6a),
        Instr::I32Sub => out.push(0x6b),
        Instr::I32Mul => out.push(0x6c),
        Instr::I32And => out.push(0x71),
        Instr::I32Or => out.push(0x72),
        Instr::I32Xor => out.push(0x73),
        Instr::I64Add => out.push(0x7c),
        Instr::I64Sub => out.push(0x7d),
        Instr::I64Mul => out.push(0x7e),
        Instr::I64And => out.push(0x83),
        Instr::I64Or => out.push(0x84),
        Instr::I64Xor => out.push(0x85),
        Instr::I32WrapI64 => out.push(0xa7),
        Instr::I64ExtendI32S => out.push(0xac),
        Instr::I64ExtendI32U => out.push(0xad),
    }
}

fn read_u32(bytes: &[u8], mut cursor: usize) -> Result<(u32, usize), WasmTrap> {
    let mut result = 0_u32;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "truncated WASM unsigned integer",
            )
        })?;
        cursor += 1;
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, cursor));
        }
        shift += 7;
        if shift > 28 {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM unsigned integer is too large",
            ));
        }
    }
}

fn read_i32(bytes: &[u8], mut cursor: usize) -> Result<(i32, usize), WasmTrap> {
    let mut result = 0_i32;
    let mut shift = 0;
    loop {
        let byte = *bytes.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "truncated WASM signed integer",
            )
        })?;
        cursor += 1;
        result |= i32::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 32 && byte & 0x40 != 0 {
                result |= !0 << shift;
            }
            return Ok((result, cursor));
        }
        if shift >= 32 {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM signed integer is too large",
            ));
        }
    }
}

fn read_i64(bytes: &[u8], mut cursor: usize) -> Result<(i64, usize), WasmTrap> {
    let mut result = 0_i64;
    let mut shift = 0;
    loop {
        let byte = *bytes
            .get(cursor)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "truncated WASM signed i64"))?;
        cursor += 1;
        result |= i64::from(byte & 0x7f) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 64 && byte & 0x40 != 0 {
                result |= !0 << shift;
            }
            return Ok((result, cursor));
        }
        if shift >= 64 {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM signed i64 is too large",
            ));
        }
    }
}

type FuncType = (Vec<ValType>, Vec<ValType>);
type FuncBody = (Vec<ValType>, Vec<Instr>);

fn decode_types(payload: &[u8]) -> Result<Vec<FuncType>, WasmTrap> {
    let (count, mut cursor) = read_u32(payload, 0)?;
    let mut types = Vec::new();
    for _ in 0..count {
        if payload.get(cursor) != Some(&0x60) {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM type is not a function type",
            ));
        }
        cursor += 1;
        let (params, next) = decode_valtypes(payload, cursor)?;
        cursor = next;
        let (results, next) = decode_valtypes(payload, cursor)?;
        cursor = next;
        types.push((params, results));
    }
    Ok(types)
}

fn decode_valtypes(payload: &[u8], cursor: usize) -> Result<(Vec<ValType>, usize), WasmTrap> {
    let (count, mut cursor) = read_u32(payload, cursor)?;
    let mut types = Vec::new();
    for _ in 0..count {
        let byte = *payload
            .get(cursor)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "truncated WASM value type"))?;
        cursor += 1;
        types.push(ValType::from_byte(byte).ok_or_else(|| {
            trap(
                ExecutionStatus::Unsupported,
                format!("unsupported WASM value type {byte:#x}"),
            )
        })?);
    }
    Ok((types, cursor))
}

fn decode_func_types(payload: &[u8]) -> Result<Vec<u32>, WasmTrap> {
    let (count, mut cursor) = read_u32(payload, 0)?;
    let mut indexes = Vec::new();
    for _ in 0..count {
        let (index, next) = read_u32(payload, cursor)?;
        cursor = next;
        indexes.push(index);
    }
    Ok(indexes)
}

fn decode_exports(payload: &[u8]) -> Result<Vec<(String, u32)>, WasmTrap> {
    let (count, mut cursor) = read_u32(payload, 0)?;
    let mut exports = Vec::new();
    for _ in 0..count {
        let (len, next) = read_u32(payload, cursor)?;
        cursor = next;
        let end = cursor + len as usize;
        let name = std::str::from_utf8(payload.get(cursor..end).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "truncated WASM export name",
            )
        })?)
        .map_err(|_| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM export name is not UTF-8",
            )
        })?
        .to_owned();
        cursor = end;
        if payload.get(cursor) != Some(&0x00) {
            return Err(trap(
                ExecutionStatus::Unsupported,
                "only function exports are supported",
            ));
        }
        cursor += 1;
        let (index, next) = read_u32(payload, cursor)?;
        cursor = next;
        exports.push((name, index));
    }
    Ok(exports)
}

fn decode_bodies(payload: &[u8]) -> Result<Vec<FuncBody>, WasmTrap> {
    let (count, mut cursor) = read_u32(payload, 0)?;
    let mut bodies = Vec::new();
    for _ in 0..count {
        let (size, next) = read_u32(payload, cursor)?;
        cursor = next;
        let end = cursor + size as usize;
        let body = payload
            .get(cursor..end)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "truncated WASM code body"))?;
        bodies.push(decode_body(body)?);
        cursor = end;
    }
    Ok(bodies)
}

fn decode_body(payload: &[u8]) -> Result<FuncBody, WasmTrap> {
    let (local_groups, mut cursor) = read_u32(payload, 0)?;
    let mut locals = Vec::new();
    for _ in 0..local_groups {
        let (count, next) = read_u32(payload, cursor)?;
        cursor = next;
        let byte = *payload
            .get(cursor)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "truncated WASM local type"))?;
        cursor += 1;
        let ty = ValType::from_byte(byte).ok_or_else(|| {
            trap(
                ExecutionStatus::Unsupported,
                format!("unsupported WASM local type {byte:#x}"),
            )
        })?;
        locals.extend(std::iter::repeat(ty).take(count as usize));
    }
    let mut body = Vec::new();
    while cursor < payload.len() {
        if payload[cursor] == 0x0b && cursor + 1 == payload.len() {
            break;
        }
        let (instr, next) = decode_instr(payload, cursor)?;
        body.push(instr);
        cursor = next;
    }
    Ok((locals, body))
}

fn decode_instr(payload: &[u8], cursor: usize) -> Result<(Instr, usize), WasmTrap> {
    let opcode = *payload.get(cursor).ok_or_else(|| {
        trap(
            ExecutionStatus::InvalidRequest,
            "truncated WASM instruction",
        )
    })?;
    let mut cursor = cursor + 1;
    let instr = match opcode {
        0x00 => Instr::Unreachable,
        0x01 => Instr::Nop,
        0x03 => {
            if payload.get(cursor) != Some(&0x40) {
                return Err(trap(
                    ExecutionStatus::Unsupported,
                    "typed WASM loops are unsupported",
                ));
            }
            cursor += 1;
            Instr::Loop
        }
        0x04 => {
            if payload.get(cursor) != Some(&0x40) {
                return Err(trap(
                    ExecutionStatus::Unsupported,
                    "typed WASM if is unsupported",
                ));
            }
            cursor += 1;
            Instr::If
        }
        0x05 => Instr::Else,
        0x0b => Instr::End,
        0x0c => {
            let (depth, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::Br(depth)
        }
        0x0f => Instr::Return,
        0x10 => {
            let (index, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::Call(index)
        }
        0x1a => Instr::Drop,
        0x20 => {
            let (index, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::LocalGet(index)
        }
        0x21 => {
            let (index, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::LocalSet(index)
        }
        0x41 => {
            let (value, next) = read_i32(payload, cursor)?;
            cursor = next;
            Instr::I32Const(value)
        }
        0x42 => {
            let (value, next) = read_i64(payload, cursor)?;
            cursor = next;
            Instr::I64Const(value)
        }
        0x45 => Instr::I32Eqz,
        0x46 => Instr::I32Eq,
        0x47 => Instr::I32Ne,
        0x48 => Instr::I32LtS,
        0x49 => Instr::I32LtU,
        0x4a => Instr::I32GtS,
        0x4b => Instr::I32GtU,
        0x4c => Instr::I32LeS,
        0x4d => Instr::I32LeU,
        0x4e => Instr::I32GeS,
        0x4f => Instr::I32GeU,
        0x51 => Instr::I64Eq,
        0x52 => Instr::I64Ne,
        0x53 => Instr::I64LtS,
        0x54 => Instr::I64LtU,
        0x55 => Instr::I64GtS,
        0x56 => Instr::I64GtU,
        0x57 => Instr::I64LeS,
        0x58 => Instr::I64LeU,
        0x59 => Instr::I64GeS,
        0x5a => Instr::I64GeU,
        0x6a => Instr::I32Add,
        0x6b => Instr::I32Sub,
        0x6c => Instr::I32Mul,
        0x71 => Instr::I32And,
        0x72 => Instr::I32Or,
        0x73 => Instr::I32Xor,
        0x7c => Instr::I64Add,
        0x7d => Instr::I64Sub,
        0x7e => Instr::I64Mul,
        0x83 => Instr::I64And,
        0x84 => Instr::I64Or,
        0x85 => Instr::I64Xor,
        0xa7 => Instr::I32WrapI64,
        0xac => Instr::I64ExtendI32S,
        0xad => Instr::I64ExtendI32U,
        other => {
            return Err(trap(
                ExecutionStatus::Unsupported,
                format!("unsupported WASM opcode {other:#x}"),
            ))
        }
    };
    Ok((instr, cursor))
}
