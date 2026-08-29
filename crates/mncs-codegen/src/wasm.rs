//! A bounded WebAssembly MVP encoder and interpreter.
//!
//! This is a research execution envelope for the supported integer/control-flow
//! subset. It is not a general WASM engine and does not import host functions.

use std::collections::BTreeMap;

use mncs_model::{ExecutionStatus, ExecutionValue, IntegerType, SemanticId};

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
    I64Eqz,
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
    Select,
    I64Mul,
    I64And,
    I64Or,
    I64Xor,
    I64ShrS,
    I32Shl,
    I32ShrU,
    I32ShrS,
    I64Shl,
    I64ShrU,
    I32WrapI64,
    I64ExtendI32S,
    I64ExtendI32U,
    I32Load,
    I32Load8U,
    I64Load,
    I32Store,
    I64Store,
    GlobalGet(u32),
    GlobalSet(u32),
    I32DivS,
    I32DivU,
    I32RemS,
    I32RemU,
    I64DivS,
    I64DivU,
    I64RemS,
    I64RemU,
    /// Lowering-time marker for the arena allocator call; never encoded.
    AllocCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmFunction {
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
    pub locals: Vec<ValType>,
    pub body: Vec<Instr>,
}

/// One linear-memory declaration. MNCS composites (records and payload-bearing
/// finite variants) are realized as immutable cells in this memory; the
/// artifact stays standard WASM MVP plus loads/stores/globals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmMemory {
    pub min_pages: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmGlobal {
    pub valtype: ValType,
    pub mutable: bool,
    pub init: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub functions: Vec<WasmFunction>,
    pub memory: Option<WasmMemory>,
    pub globals: Vec<WasmGlobal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmTrap {
    pub status: ExecutionStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExecution {
    pub returned: Vec<ExecutionValue>,
    pub steps: u64,
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
    if let Some(memory) = &module.memory {
        emit_section(&mut bytes, 5, |section| {
            encode_u32(section, 1);
            section.push(0x00);
            encode_u32(section, memory.min_pages);
        });
    }
    if !module.globals.is_empty() {
        emit_section(&mut bytes, 6, |section| {
            encode_u32(section, module.globals.len() as u32);
            for global in &module.globals {
                section.push(global.valtype.byte());
                section.push(u8::from(global.mutable));
                match global.valtype {
                    ValType::I32 => {
                        section.push(0x41);
                        encode_i32(section, global.init as i32);
                    }
                    ValType::I64 => {
                        section.push(0x42);
                        encode_i64(section, global.init);
                    }
                }
                section.push(0x0b);
            }
        });
    }
    emit_section(&mut bytes, 7, |section| {
        let export_count = module.functions.len() as u32 + u32::from(module.memory.is_some());
        encode_u32(section, export_count);
        for (index, function) in module.functions.iter().enumerate() {
            encode_name(section, &function.name);
            section.push(0x00);
            encode_u32(section, index as u32);
        }
        if module.memory.is_some() {
            // Browser hosts need a standard named memory export to populate
            // packed view descriptors. The embedded interpreter ignores this
            // non-function export while retaining the same module bytes.
            encode_name(section, "memory");
            section.push(0x02);
            encode_u32(section, 0);
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
    let mut memory = None;
    let mut globals = Vec::new();
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
            5 => memory = decode_memory(payload)?,
            6 => globals = decode_globals(payload)?,
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
    Ok(WasmModule {
        functions,
        memory,
        globals,
    })
}

fn decode_memory(payload: &[u8]) -> Result<Option<WasmMemory>, WasmTrap> {
    let (count, cursor) = read_u32(payload, 0)?;
    if count != 1 {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "WASM memory section must declare exactly one memory",
        ));
    }
    let flags = *payload.get(cursor).ok_or_else(|| {
        trap(
            ExecutionStatus::InvalidRequest,
            "WASM memory limits truncated",
        )
    })?;
    if flags != 0x00 {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "WASM memory maximum/imports are outside the MVP subset",
        ));
    }
    let (min_pages, _) = read_u32(payload, cursor + 1)?;
    Ok(Some(WasmMemory { min_pages }))
}

fn decode_globals(payload: &[u8]) -> Result<Vec<WasmGlobal>, WasmTrap> {
    let (count, mut cursor) = read_u32(payload, 0)?;
    let mut globals = Vec::new();
    for _ in 0..count {
        let valtype_byte = *payload.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM global type truncated",
            )
        })?;
        let valtype = ValType::from_byte(valtype_byte).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM global has an unsupported value type",
            )
        })?;
        let mutable = *payload.get(cursor + 1).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM global mutability truncated",
            )
        })? == 0x01;
        cursor += 2;
        // init expression: const opcode + value + end
        let opcode = *payload.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM global init truncated",
            )
        })?;
        let init = match (opcode, valtype) {
            (0x41, ValType::I32) => {
                let (value, next) = read_i32(payload, cursor + 1)?;
                cursor = next;
                i64::from(value)
            }
            (0x42, ValType::I64) => {
                let (value, next) = read_i64(payload, cursor + 1)?;
                cursor = next;
                value
            }
            _ => {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "WASM global init expression is unsupported",
                ))
            }
        };
        if *payload.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "WASM global init missing end",
            )
        })? != 0x0b
        {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "WASM global init expression is not terminated",
            ));
        }
        cursor += 1;
        globals.push(WasmGlobal {
            valtype,
            mutable,
            init,
        });
    }
    Ok(globals)
}

/// Per-invocation linear memory + globals for one module execution.
#[derive(Debug)]
pub struct Runtime {
    memory: Vec<u8>,
    globals: Vec<i64>,
}

impl Runtime {
    pub fn new(module: &WasmModule) -> Self {
        let pages = module
            .memory
            .as_ref()
            .map(|memory| memory.min_pages)
            .unwrap_or(0);
        Self {
            memory: vec![0_u8; pages as usize * 65_536],
            globals: module.globals.iter().map(|global| global.init).collect(),
        }
    }

    fn load(&self, address: i64, width: usize) -> Result<u64, WasmTrap> {
        let base = usize::try_from(address)
            .map_err(|_| trap(ExecutionStatus::InvalidRequest, "negative memory address"))?;
        let end = base
            .checked_add(width)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "memory address overflow"))?;
        if end > self.memory.len() {
            return Err(trap(
                ExecutionStatus::RuntimeFailure,
                "backend trap: out-of-bounds memory load",
            ));
        }
        let mut value = 0_u64;
        for (index, byte) in self.memory[base..end].iter().enumerate() {
            value |= u64::from(*byte) << (8 * index);
        }
        Ok(value)
    }

    fn store(&mut self, address: i64, width: usize, value: u64) -> Result<(), WasmTrap> {
        let base = usize::try_from(address)
            .map_err(|_| trap(ExecutionStatus::InvalidRequest, "negative memory address"))?;
        let end = base
            .checked_add(width)
            .ok_or_else(|| trap(ExecutionStatus::InvalidRequest, "memory address overflow"))?;
        if end > self.memory.len() {
            return Err(trap(
                ExecutionStatus::RuntimeFailure,
                "backend trap: out-of-bounds memory store",
            ));
        }
        for index in 0..width {
            self.memory[base + index] = ((value >> (8 * index)) & 0xff) as u8;
        }
        Ok(())
    }

    /// Bump allocation for immutable composite cells; alignment is 8 bytes so
    /// every slot can hold an i64 regardless of its logical field width.
    fn allocate(&mut self, bytes: u32) -> Result<i64, WasmTrap> {
        const BUMP_GLOBAL: u32 = 0;
        let current = self.globals.get(BUMP_GLOBAL as usize).copied().unwrap_or(8);
        let aligned = i64::from(bytes.div_ceil(8) * 8);
        let next = current
            .checked_add(aligned)
            .ok_or_else(|| trap(ExecutionStatus::RuntimeFailure, "arena pointer overflow"))?;
        if usize::try_from(next)
            .map(|next| next > self.memory.len())
            .unwrap_or(true)
        {
            return Err(trap(
                ExecutionStatus::BudgetExhausted,
                "composite arena exhausted under this step budget",
            ));
        }
        if let Some(slot) = self.globals.get_mut(BUMP_GLOBAL as usize) {
            *slot = next;
        }
        Ok(current)
    }
}

fn execute_raw(
    module: &WasmModule,
    runtime: &mut Runtime,
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
        if std::env::var_os("MNCS_WASM_TRACE").is_some() {
            eprintln!(
                "wasm-trace {} {} {:?}",
                function.name, ip, function.body[ip]
            );
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
                        format!(
                            "backend call operand stack underflow: callee {} wants {} params, stack holds {}",
                            callee.name,
                            callee.params.len(),
                            stack.len()
                        ),
                    ));
                }
                let arguments = stack.split_off(stack.len() - callee.params.len());
                let returned = execute_raw(
                    module,
                    runtime,
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
            Instr::I32Shl => bin_i32(&mut stack, |left, right| {
                let count = right % 32;
                left.wrapping_shl(count as u32)
            })?,
            Instr::I32ShrU => bin_i32(&mut stack, |left, right| {
                let count = right % 32;
                ((left as u32).wrapping_shr(count as u32)) as i32
            })?,
            Instr::I32ShrS => bin_i32(&mut stack, |left, right| {
                let count = right % 32;
                left.wrapping_shr(count as u32)
            })?,
            Instr::I64Shl => bin_i64(&mut stack, |left, right| {
                let count = (right as u64 % 64) as u32;
                left.wrapping_shl(count)
            })?,
            Instr::I64ShrU => bin_i64(&mut stack, |left, right| {
                let count = (right as u64 % 64) as u32;
                ((left as u64).wrapping_shr(count)) as i64
            })?,
            Instr::I64ShrS => bin_i64(&mut stack, |left, right| {
                left.wrapping_shr(u32::try_from(right & 63).unwrap_or(0))
            })?,
            Instr::I64Eqz => {
                let value = pop(&mut stack)?;
                stack.push(i64::from(value == 0));
            }
            Instr::Select => {
                let condition = pop_i32(&mut stack)?;
                let false_value = pop(&mut stack)?;
                let true_value = pop(&mut stack)?;
                stack.push(if condition != 0 {
                    true_value
                } else {
                    false_value
                });
            }
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
            Instr::I32Load => {
                let address = pop_i32(&mut stack)?;
                let raw = runtime.load(i64::from(address), 4)?;
                stack.push(i64::from(raw as u32 as i32));
            }
            Instr::I32Load8U => {
                let address = pop_i32(&mut stack)?;
                let raw = runtime.load(i64::from(address), 1)?;
                stack.push(i64::from(raw as u8));
            }
            Instr::I64Load => {
                let address = pop_i32(&mut stack)?;
                let raw = runtime.load(i64::from(address), 8)?;
                stack.push(raw as i64);
            }
            Instr::I32Store => {
                let value = pop_i32(&mut stack)?;
                let address = pop_i32(&mut stack)?;
                runtime.store(i64::from(address), 4, u64::from(value as u32))?;
            }
            Instr::I64Store => {
                let value = pop(&mut stack)?;
                let address = pop_i32(&mut stack)?;
                runtime.store(i64::from(address), 8, value as u64)?;
            }
            Instr::GlobalGet(index) => {
                let value = runtime
                    .globals
                    .get(*index as usize)
                    .copied()
                    .ok_or_else(|| {
                        trap(
                            ExecutionStatus::InvalidRequest,
                            "global.get index is out of range",
                        )
                    })?;
                stack.push(value);
            }
            Instr::GlobalSet(index) => {
                let value = pop(&mut stack)?;
                let slot = runtime.globals.get_mut(*index as usize).ok_or_else(|| {
                    trap(
                        ExecutionStatus::InvalidRequest,
                        "global.set index is out of range",
                    )
                })?;
                *slot = value;
            }
            // WASM division traps on zero and INT_MIN / -1; both surface here
            // as explicit MNCS runtime failures under the same conditions the
            // reference executors reject them.
            Instr::I32DivS => bin_try_i32(&mut stack, |left, right| {
                if right == 0 || (left == i32::MIN && right == -1) {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: signed integer division by zero or overflow",
                    ));
                }
                Ok(left / right)
            })?,
            // Unsigned division has no MIN / -1 overflow case; only a zero
            // divisor fails, matching the reference executors.
            Instr::I32DivU => bin_try_i32(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: unsigned integer division by zero",
                    ));
                }
                Ok((left as u32 / right as u32) as i32)
            })?,
            Instr::I32RemS => bin_try_i32(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: integer remainder by zero",
                    ));
                }
                Ok(left.wrapping_rem(right))
            })?,
            Instr::I32RemU => bin_try_i32(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: integer remainder by zero",
                    ));
                }
                Ok((left as u32 % right as u32) as i32)
            })?,
            Instr::I64DivS => bin_try_i64(&mut stack, |left, right| {
                if right == 0 || (left == i64::MIN && right == -1) {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: signed integer division by zero or overflow",
                    ));
                }
                Ok(left / right)
            })?,
            Instr::I64DivU => bin_try_i64(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: unsigned integer division by zero",
                    ));
                }
                Ok((left as u64 / right as u64) as i64)
            })?,
            Instr::I64RemS => bin_try_i64(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: integer remainder by zero",
                    ));
                }
                Ok(left.wrapping_rem(right))
            })?,
            Instr::I64RemU => bin_try_i64(&mut stack, |left, right| {
                if right == 0 {
                    return Err(trap(
                        ExecutionStatus::RuntimeFailure,
                        "backend trap: integer remainder by zero",
                    ));
                }
                Ok((left as u64 % right as u64) as i64)
            })?,
            Instr::AllocCall => {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "AllocCall marker survived into execution",
                ))
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

fn bin_try_i32(
    stack: &mut Vec<i64>,
    op: impl Fn(i32, i32) -> Result<i32, WasmTrap>,
) -> Result<(), WasmTrap> {
    let right = pop_i32(stack)?;
    let left = pop_i32(stack)?;
    stack.push(i64::from(op(left, right)?));
    Ok(())
}

fn bin_try_i64(
    stack: &mut Vec<i64>,
    op: impl Fn(i64, i64) -> Result<i64, WasmTrap>,
) -> Result<(), WasmTrap> {
    let right = pop(stack)?;
    let left = pop(stack)?;
    stack.push(op(left, right)?);
    Ok(())
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

/// Logical marshaling descriptors for exported signatures. They carry the
/// language-owned identities so backend observations are indistinguishable
/// from reference-executor values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarshalTy {
    Bool,
    Int(IntegerType),
    /// Payload-free finite value carried directly as its discriminant.
    BareFinite {
        type_identity: SemanticId,
        variants: BTreeMap<u32, SemanticId>,
    },
    /// Payload-bearing finite value carried as a pointer to a tag cell plus
    /// one 8-byte slot per payload field of the constructed variant.
    BoxedFinite(Box<BoxedFiniteTy>),
    /// Record value carried as a pointer to canonical field cells.
    Record(Box<RecordTy>),
    /// Exact-length sequence: i32 cell pointer, 8-byte slots.
    Sequence {
        element: Box<MarshalTy>,
        length: u32,
    },
    /// Bounded view: packed i64 descriptor. Byte views use packed bytes;
    /// other elements currently use an exact eight-byte-slot cell.
    View {
        element: Box<MarshalTy>,
        capacity: u32,
    },
    /// Integer vector. Portable WASM stores packed lane widths; the
    /// ABI value is an i32 cell pointer.
    Vector {
        element: IntegerType,
        lanes: u32,
    },
    /// Packed predicate bits in an i64 word.
    Mask {
        lanes: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxedFiniteTy {
    pub type_identity: SemanticId,
    pub variants: BTreeMap<u32, FiniteVariantTy>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteVariantTy {
    pub variant_identity: SemanticId,
    pub fields: Vec<(String, MarshalTy)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTy {
    pub type_identity: SemanticId,
    pub name: String,
    pub fields: Vec<(String, MarshalTy)>,
}

impl MarshalTy {
    fn is_boxed(&self) -> bool {
        matches!(
            self,
            Self::BoxedFinite(_) | Self::Record(_) | Self::Sequence { .. } | Self::Vector { .. }
        )
    }
}

fn write_marshal(
    runtime: &mut Runtime,
    value: &ExecutionValue,
    ty: &MarshalTy,
) -> Result<i64, WasmTrap> {
    match (value, ty) {
        (ExecutionValue::Boolean { value }, MarshalTy::Bool) => Ok(i64::from(*value)),
        (ExecutionValue::Integer { value, .. }, MarshalTy::Int(integer)) => {
            if !crate::support::integer_fits(*value, *integer) {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "argument is outside the declared integer type",
                ));
            }
            Ok(*value as i64)
        }
        // Bytes marshal through their unsigned 8-bit domain.
        (ExecutionValue::Byte { value }, MarshalTy::Int(integer))
            if integer.bits == 8 && !integer.signed =>
        {
            if !(0..=255).contains(value) {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "argument is outside the byte domain",
                ));
            }
            Ok(*value as i64)
        }
        (ExecutionValue::Finite { discriminant, .. }, MarshalTy::BareFinite { .. }) => {
            Ok(i64::from(*discriminant))
        }
        (value @ ExecutionValue::Finite { .. }, MarshalTy::BoxedFinite(layout)) => {
            let discriminant = match value {
                ExecutionValue::Finite { discriminant, .. } => *discriminant,
                _ => unreachable!("matched above"),
            };
            let variant = layout.variants.get(&discriminant).ok_or_else(|| {
                trap(
                    ExecutionStatus::InvalidRequest,
                    "finite argument has no declared payload layout",
                )
            })?;
            let payload = match value {
                ExecutionValue::Finite { payload, .. } => payload.clone(),
                _ => Vec::new(),
            };
            if variant.fields.len() != payload.len() {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "finite argument payload does not match the declared layout",
                ));
            }
            let address = runtime.allocate(((variant.fields.len() as u32) + 1) * 8)?;
            runtime.store(address, 4, u64::from(discriminant))?;
            for (index, (_, field_ty)) in variant.fields.iter().enumerate() {
                let slot = address + ((index + 1) * 8) as i64;
                write_slot(runtime, &payload[index].1, field_ty, slot)?;
            }
            Ok(address)
        }
        (value @ ExecutionValue::Record { .. }, MarshalTy::Record(layout)) => {
            let fields = match value {
                ExecutionValue::Record { fields, .. } => fields.clone(),
                _ => unreachable!("matched above"),
            };
            if layout.fields.len() != fields.len() {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "record argument does not match the declared field count",
                ));
            }
            let address = runtime.allocate((layout.fields.len() as u32) * 8)?;
            for (index, (_, field_ty)) in layout.fields.iter().enumerate() {
                let slot = address + (index * 8) as i64;
                write_slot(runtime, &fields[index].1, field_ty, slot)?;
            }
            Ok(address)
        }
        (ExecutionValue::Sequence { values }, MarshalTy::Sequence { element, length }) => {
            if values.len() != *length as usize {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "sequence argument does not match its declared length",
                ));
            }
            write_sequence_cell(runtime, values, element)
        }
        (ExecutionValue::Sequence { values }, MarshalTy::View { element, capacity }) => {
            if values.len() > *capacity as usize {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "view argument exceeds its declared capacity",
                ));
            }
            if values.is_empty() {
                return Ok(0);
            }
            let address = if is_byte_marshal_ty(element) {
                write_byte_view(runtime, values)?
            } else {
                write_sequence_cell(runtime, values, element)?
            };
            let packed = (address as u32 as u64) | ((values.len() as u64) << 32);
            Ok(packed as i64)
        }
        (ExecutionValue::Vector { values }, MarshalTy::Vector { element, lanes }) => {
            if values.len() != *lanes as usize {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "vector argument does not match its declared lane count",
                ));
            }
            write_vector_cell(runtime, values, *element, *lanes)
        }
        (ExecutionValue::Mask { lanes: bits }, MarshalTy::Mask { lanes }) => {
            if bits.len() != *lanes as usize {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "mask argument does not match its declared lane count",
                ));
            }
            Ok(crate::composite::pack_mask(bits) as i64)
        }
        _ => Err(trap(
            ExecutionStatus::InvalidRequest,
            "argument does not match the exported logical type",
        )),
    }
}

fn write_sequence_cell(
    runtime: &mut Runtime,
    values: &[ExecutionValue],
    element: &MarshalTy,
) -> Result<i64, WasmTrap> {
    let address = runtime.allocate((values.len() as u32) * 8)?;
    for (index, value) in values.iter().enumerate() {
        let slot = address + (index * 8) as i64;
        write_slot(runtime, value, element, slot)?;
    }
    Ok(address)
}

fn write_vector_cell(
    runtime: &mut Runtime,
    values: &[ExecutionValue],
    element: IntegerType,
    lanes: u32,
) -> Result<i64, WasmTrap> {
    let lane_bytes: u32 = if element.bits == 64 { 8 } else { 4 };
    let address = runtime.allocate(lanes * lane_bytes)?;
    let width = lane_bytes as usize;
    for (index, value) in values.iter().enumerate() {
        let bits = match value {
            ExecutionValue::Integer { value, .. } => *value as u64,
            _ => {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "vector lane is not an integer",
                ))
            }
        };
        runtime.store(address + i64::from(index as u32 * lane_bytes), width, bits)?;
    }
    Ok(address)
}

fn write_slot(
    runtime: &mut Runtime,
    value: &ExecutionValue,
    ty: &MarshalTy,
    address: i64,
) -> Result<(), WasmTrap> {
    if ty.is_boxed()
        && matches!(
            value,
            ExecutionValue::Record { .. }
                | ExecutionValue::Finite { .. }
                | ExecutionValue::Sequence { .. }
                | ExecutionValue::Vector { .. }
        )
    {
        let pointer = write_marshal(runtime, value, ty)?;
        return runtime.store(address, 4, pointer as u32 as u64);
    }
    match (value, ty) {
        (ExecutionValue::Boolean { value }, MarshalTy::Bool) => {
            runtime.store(address, 4, u64::from(*value))
        }
        (ExecutionValue::Integer { value, .. }, MarshalTy::Int(integer)) => {
            if integer.bits == 64 {
                runtime.store(address, 8, *value as u64)
            } else {
                runtime.store(address, 4, (*value as i32) as u32 as u64)
            }
        }
        (ExecutionValue::Finite { discriminant, .. }, MarshalTy::BareFinite { .. }) => {
            runtime.store(address, 4, u64::from(*discriminant))
        }
        (ExecutionValue::Byte { value }, MarshalTy::Int(integer))
            if integer.bits == 8 && !integer.signed =>
        {
            runtime.store(address, 4, *value as u64)
        }
        (ExecutionValue::Mask { lanes }, MarshalTy::Mask { .. }) => {
            runtime.store(address, 8, crate::composite::pack_mask(lanes))
        }
        (ExecutionValue::Sequence { values }, MarshalTy::View { element, capacity }) => {
            if values.len() > *capacity as usize {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "view field exceeds its declared capacity",
                ));
            }
            let packed = if values.is_empty() {
                0
            } else if is_byte_marshal_ty(element) {
                let address = write_byte_view(runtime, values)?;
                (address as u32 as u64) | ((values.len() as u64) << 32)
            } else {
                let cell = write_sequence_cell(runtime, values, element)?;
                (cell as u32 as u64) | ((values.len() as u64) << 32)
            };
            runtime.store(address, 8, packed)
        }
        _ => Err(trap(
            ExecutionStatus::InvalidRequest,
            "composite field does not match its declared logical type",
        )),
    }
}

fn is_byte_marshal_ty(ty: &MarshalTy) -> bool {
    matches!(
        ty,
        MarshalTy::Int(IntegerType {
            bits: 8,
            signed: false
        })
    )
}

fn write_byte_view(runtime: &mut Runtime, values: &[ExecutionValue]) -> Result<i64, WasmTrap> {
    let address = runtime.allocate(values.len() as u32)?;
    for (index, value) in values.iter().enumerate() {
        let ExecutionValue::Byte { value } = value else {
            return Err(trap(
                ExecutionStatus::InvalidRequest,
                "byte view contains a non-byte value",
            ));
        };
        runtime.store(address + index as i64, 1, *value as u64)?;
    }
    Ok(address)
}

fn read_slot(runtime: &Runtime, ty: &MarshalTy, address: i64) -> Result<ExecutionValue, WasmTrap> {
    match ty {
        MarshalTy::Bool => Ok(ExecutionValue::Boolean {
            value: runtime.load(address, 4)? == 1,
        }),
        MarshalTy::Int(integer) => {
            let width = if integer.bits == 64 { 8 } else { 4 };
            let raw = runtime.load(address, width)?;
            if integer.bits == 8 && !integer.signed {
                return Ok(ExecutionValue::Byte {
                    value: i128::from(raw as u8),
                });
            }
            Ok(ExecutionValue::Integer {
                value: crate::composite::integer_from_slot_bits(raw, *integer),
                ty: *integer,
            })
        }
        MarshalTy::BareFinite {
            type_identity,
            variants,
        } => {
            let discriminant = runtime.load(address, 4)? as u32;
            let variant_identity = variants
                .get(&discriminant)
                .cloned()
                .unwrap_or_else(|| SemanticId("unresolved".to_owned()));
            Ok(ExecutionValue::Finite {
                type_identity: type_identity.clone(),
                variant_identity,
                discriminant,
                payload: Vec::new(),
            })
        }
        MarshalTy::BoxedFinite(_) | MarshalTy::Record(_) => {
            let pointer = runtime.load(address, 4)? as u32 as i64;
            read_marshal(runtime, ty, pointer)
        }
        MarshalTy::Sequence { .. } | MarshalTy::Vector { .. } => {
            let pointer = runtime.load(address, 4)? as u32 as i64;
            read_marshal(runtime, ty, pointer)
        }
        MarshalTy::View { .. } => {
            let descriptor = runtime.load(address, 8)? as i64;
            read_marshal(runtime, ty, descriptor)
        }
        MarshalTy::Mask { lanes } => Ok(crate::composite::unpack_mask(
            runtime.load(address, 8)?,
            *lanes,
        )),
    }
}

fn read_marshal(
    runtime: &Runtime,
    ty: &MarshalTy,
    address: i64,
) -> Result<ExecutionValue, WasmTrap> {
    match ty {
        MarshalTy::BoxedFinite(layout) => {
            let tag = runtime.load(address, 4)? as u32;
            let variant = layout.variants.get(&tag).ok_or_else(|| {
                trap(
                    ExecutionStatus::RuntimeFailure,
                    "backend returned an unknown finite discriminant",
                )
            })?;
            let mut payload = Vec::new();
            for (index, (name, field_ty)) in variant.fields.iter().enumerate() {
                let slot = address + ((index + 1) * 8) as i64;
                payload.push((name.clone(), read_slot(runtime, field_ty, slot)?));
            }
            Ok(ExecutionValue::Finite {
                type_identity: layout.type_identity.clone(),
                variant_identity: variant.variant_identity.clone(),
                discriminant: tag,
                payload,
            })
        }
        MarshalTy::Record(layout) => {
            let mut values = Vec::new();
            for (index, (name, field_ty)) in layout.fields.iter().enumerate() {
                let slot = address + (index * 8) as i64;
                values.push((name.clone(), read_slot(runtime, field_ty, slot)?));
            }
            Ok(ExecutionValue::Record {
                type_identity: layout.type_identity.clone(),
                name: layout.name.clone(),
                fields: values,
            })
        }
        MarshalTy::Sequence { element, length } => {
            read_sequence_cell(runtime, address, element, *length)
        }
        MarshalTy::View { element, capacity } => {
            let (offset, length) = crate::composite::unpack_view(address as u64);
            if length > *capacity {
                return Err(trap(
                    ExecutionStatus::RuntimeFailure,
                    "backend returned a view longer than its declared capacity",
                ));
            }
            if is_byte_marshal_ty(element) {
                read_byte_view(runtime, offset as i64, length)
            } else {
                read_sequence_cell(runtime, offset as i64, element, length)
            }
        }
        MarshalTy::Vector { element, lanes } => {
            read_vector_cell(runtime, address, *element, *lanes)
        }
        MarshalTy::Mask { lanes } => Ok(crate::composite::unpack_mask(address as u64, *lanes)),
        other => read_slot(runtime, other, address),
    }
}

fn read_byte_view(
    runtime: &Runtime,
    address: i64,
    length: u32,
) -> Result<ExecutionValue, WasmTrap> {
    let mut values = Vec::with_capacity(length as usize);
    for index in 0..length {
        values.push(ExecutionValue::Byte {
            value: i128::from(runtime.load(address + i64::from(index), 1)? as u8),
        });
    }
    Ok(ExecutionValue::Sequence { values })
}

fn read_sequence_cell(
    runtime: &Runtime,
    address: i64,
    element: &MarshalTy,
    length: u32,
) -> Result<ExecutionValue, WasmTrap> {
    let mut values = Vec::with_capacity(length as usize);
    for index in 0..length {
        let slot = address + i64::from(index) * 8;
        values.push(read_slot(runtime, element, slot)?);
    }
    Ok(ExecutionValue::Sequence { values })
}

fn read_vector_cell(
    runtime: &Runtime,
    address: i64,
    element: IntegerType,
    lanes: u32,
) -> Result<ExecutionValue, WasmTrap> {
    let lane_bytes: i64 = if element.bits == 64 { 8 } else { 4 };
    let mut values = Vec::with_capacity(lanes as usize);
    for index in 0..lanes {
        let raw = runtime.load(address + i64::from(index) * lane_bytes, lane_bytes as usize)?;
        values.push(ExecutionValue::Integer {
            value: crate::composite::integer_from_slot_bits(raw, element),
            ty: element,
        });
    }
    Ok(ExecutionValue::Vector { values })
}

/// Execute an export with logical composite marshaling. Composite arguments
/// are written into linear memory; results are materialized back into
/// logical ExecutionValues carrying language-owned identities.
pub fn execute_function_typed(
    module: &WasmModule,
    name: &str,
    arguments: &[ExecutionValue],
    param_tys: &[MarshalTy],
    result_tys: &[MarshalTy],
    step_budget: u64,
) -> Result<WasmExecution, WasmTrap> {
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
    if arguments.len() != function.params.len() || param_tys.len() != function.params.len() {
        return Err(trap(
            ExecutionStatus::InvalidRequest,
            "backend argument count does not match the exported function",
        ));
    }
    let mut runtime = Runtime::new(module);
    let mut steps = 0_u64;
    let opcode_budget = step_budget.saturating_mul(32).max(1);
    let mut raw_arguments = Vec::with_capacity(arguments.len());
    for (argument, ty) in arguments.iter().zip(param_tys) {
        raw_arguments.push(write_marshal(&mut runtime, argument, ty)?);
    }
    let returned_raw = execute_raw(
        module,
        &mut runtime,
        function_index,
        raw_arguments,
        opcode_budget,
        &mut steps,
        0,
    )?;
    let mut returned = Vec::with_capacity(returned_raw.len());
    for (raw, ty) in returned_raw.iter().zip(result_tys) {
        returned.push(match ty {
            MarshalTy::Bool => ExecutionValue::Boolean { value: *raw == 1 },
            MarshalTy::Int(integer) => ExecutionValue::Integer {
                value: crate::composite::integer_from_slot_bits(*raw as u64, *integer),
                ty: *integer,
            },
            MarshalTy::BareFinite {
                type_identity,
                variants,
            } => {
                let discriminant = *raw as u32;
                let variant_identity = variants
                    .get(&discriminant)
                    .cloned()
                    .unwrap_or_else(|| SemanticId("unresolved".to_owned()));
                ExecutionValue::Finite {
                    type_identity: type_identity.clone(),
                    variant_identity,
                    discriminant,
                    payload: Vec::new(),
                }
            }
            MarshalTy::BoxedFinite(_) | MarshalTy::Record(_) => read_marshal(&runtime, ty, *raw)?,
            MarshalTy::Sequence { .. }
            | MarshalTy::View { .. }
            | MarshalTy::Vector { .. }
            | MarshalTy::Mask { .. } => read_marshal(&runtime, ty, *raw)?,
        });
    }
    Ok(WasmExecution { returned, steps })
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
        Instr::I64Eqz => out.push(0x50),
        Instr::Select => out.push(0x1b),
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
        Instr::I64ShrS => out.push(0x87),
        Instr::I32Shl => out.push(0x74),
        Instr::I32ShrU => out.push(0x76),
        Instr::I32ShrS => out.push(0x78),
        Instr::I64Shl => out.push(0x86),
        Instr::I64ShrU => out.push(0x88),
        Instr::I32WrapI64 => out.push(0xa7),
        Instr::I64ExtendI32S => out.push(0xac),
        Instr::I64ExtendI32U => out.push(0xad),
        Instr::I32Load => mem_instr(out, 0x28, 2),
        Instr::I32Load8U => mem_instr(out, 0x2d, 0),
        Instr::I64Load => mem_instr(out, 0x29, 2),
        Instr::I32Store => mem_instr(out, 0x36, 2),
        Instr::I64Store => mem_instr(out, 0x37, 2),
        Instr::GlobalGet(index) => {
            out.push(0x23);
            encode_u32(out, *index);
        }
        Instr::GlobalSet(index) => {
            out.push(0x24);
            encode_u32(out, *index);
        }
        Instr::I32DivS => out.push(0x6d),
        Instr::I32DivU => out.push(0x6e),
        Instr::I32RemS => out.push(0x6f),
        Instr::I32RemU => out.push(0x70),
        Instr::I64DivS => out.push(0x7f),
        Instr::I64DivU => out.push(0x80),
        Instr::I64RemS => out.push(0x81),
        Instr::I64RemU => out.push(0x82),
        Instr::AllocCall => unreachable!("AllocCall must be rewritten before encoding"),
    }
}

/// Memory load/store immediate with a static byte offset. The byte load has
/// alignment=0 because WASM validation rejects a wider alignment hint for it.
fn mem_instr(out: &mut Vec<u8>, opcode: u8, alignment: u8) {
    out.push(opcode);
    out.push(alignment);
    encode_u32(out, 0);
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
        let export_kind = *payload.get(cursor).ok_or_else(|| {
            trap(
                ExecutionStatus::InvalidRequest,
                "truncated WASM export kind",
            )
        })?;
        cursor += 1;
        let (index, next) = read_u32(payload, cursor)?;
        cursor = next;
        match export_kind {
            0x00 => exports.push((name, index)),
            0x02 if index == 0 => {}
            0x02 => {
                return Err(trap(
                    ExecutionStatus::InvalidRequest,
                    "WASM memory export references an unsupported memory index",
                ));
            }
            _ => {
                return Err(trap(
                    ExecutionStatus::Unsupported,
                    "only function and memory exports are supported",
                ));
            }
        }
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
        locals.extend(std::iter::repeat_n(ty, count as usize));
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

/// Memory load/store immediates carry align+offset; our emitter always writes
/// align=2 offset=0, so skip both LEB fields.
fn skip_memarg(payload: &[u8], mut cursor: usize) -> Result<usize, WasmTrap> {
    let (_, next) = read_u32(payload, cursor)?;
    cursor = next;
    let (_, next) = read_u32(payload, cursor)?;
    Ok(next)
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
        0x50 => Instr::I64Eqz,
        0x1b => Instr::Select,
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
        0x87 => Instr::I64ShrS,
        0x74 => Instr::I32Shl,
        0x76 => Instr::I32ShrU,
        0x78 => Instr::I32ShrS,
        0x86 => Instr::I64Shl,
        0x88 => Instr::I64ShrU,
        0x28 => {
            cursor = skip_memarg(payload, cursor)?;
            Instr::I32Load
        }
        0x2d => {
            cursor = skip_memarg(payload, cursor)?;
            Instr::I32Load8U
        }
        0x29 => {
            cursor = skip_memarg(payload, cursor)?;
            Instr::I64Load
        }
        0x36 => {
            cursor = skip_memarg(payload, cursor)?;
            Instr::I32Store
        }
        0x37 => {
            cursor = skip_memarg(payload, cursor)?;
            Instr::I64Store
        }
        0x23 => {
            let (index, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::GlobalGet(index)
        }
        0x24 => {
            let (index, next) = read_u32(payload, cursor)?;
            cursor = next;
            Instr::GlobalSet(index)
        }
        0x6d => Instr::I32DivS,
        0x6e => Instr::I32DivU,
        0x6f => Instr::I32RemS,
        0x70 => Instr::I32RemU,
        0x7f => Instr::I64DivS,
        0x80 => Instr::I64DivU,
        0x81 => Instr::I64RemS,
        0x82 => Instr::I64RemU,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_leb_roundtrips_integer_extremes() {
        for value in [i32::MIN, -42, -1, 0, 1, i32::MAX] {
            let mut bytes = Vec::new();
            encode_i32(&mut bytes, value);
            let (decoded, consumed) = read_i32(&bytes, 0).expect("decode i32");
            assert_eq!(decoded, value);
            assert_eq!(consumed, bytes.len());
        }
        for value in [i64::MIN, -42, -1, 0, 1, i64::MAX] {
            let mut bytes = Vec::new();
            encode_i64(&mut bytes, value);
            let (decoded, consumed) = read_i64(&bytes, 0).expect("decode i64");
            assert_eq!(decoded, value);
            assert_eq!(consumed, bytes.len());
        }
    }

    #[test]
    fn branchless_signed_max_keeps_negative_values() {
        let mut body = vec![
            Instr::I32Const(i32::MIN),
            Instr::LocalSet(0),
            // b ^ ((a ^ b) & -(a > b)), with a=MIN and b=-42.
            Instr::I32Const(-42),
            Instr::LocalGet(0),
            Instr::I32Const(-42),
            Instr::I32Xor,
            Instr::I32Const(0),
            Instr::LocalGet(0),
            Instr::I32Const(-42),
            Instr::I32GtS,
            Instr::I32Sub,
            Instr::I32And,
            Instr::I32Xor,
            Instr::LocalSet(0),
            Instr::LocalGet(0),
        ];
        let module = WasmModule {
            functions: vec![WasmFunction {
                name: "max".to_owned(),
                params: Vec::new(),
                results: vec![ValType::I32],
                locals: vec![ValType::I32],
                body: std::mem::take(&mut body),
            }],
            memory: None,
            globals: Vec::new(),
        };
        let decoded = decode_module(&encode_module(&module)).expect("decode module");
        let execution = execute_function_typed(
            &decoded,
            "max",
            &[],
            &[],
            &[MarshalTy::Int(IntegerType {
                bits: 32,
                signed: true,
            })],
            100,
        )
        .expect("execute max");
        assert_eq!(
            execution.returned,
            vec![ExecutionValue::Integer {
                value: -42,
                ty: IntegerType {
                    bits: 32,
                    signed: true,
                },
            }]
        );
    }

    #[test]
    fn memory_export_and_packed_byte_load_roundtrip() {
        let module = WasmModule {
            functions: vec![WasmFunction {
                name: "byte_roundtrip".to_owned(),
                params: Vec::new(),
                results: vec![ValType::I32],
                locals: Vec::new(),
                body: vec![
                    Instr::I32Const(0),
                    Instr::I32Const(0xab),
                    Instr::I32Store,
                    Instr::I32Const(0),
                    Instr::I32Load8U,
                ],
            }],
            memory: Some(WasmMemory { min_pages: 1 }),
            globals: Vec::new(),
        };

        let bytes = encode_module(&module);
        let decoded = decode_module(&bytes).expect("decode module with memory export");
        assert_eq!(decoded.memory, module.memory);
        let execution = execute_function_typed(
            &decoded,
            "byte_roundtrip",
            &[],
            &[],
            &[MarshalTy::Int(IntegerType {
                bits: 32,
                signed: false,
            })],
            100,
        )
        .expect("execute byte load");
        assert_eq!(
            execution.returned,
            vec![ExecutionValue::Integer {
                value: 0xab,
                ty: IntegerType {
                    bits: 32,
                    signed: false,
                },
            }]
        );
    }
}
