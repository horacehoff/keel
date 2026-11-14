use crate::{
    compiler::functions::builtin::{BuiltIn, ErrorSpan, Lowering, ReturnType, SignatureType},
    instr::Instr,
};

pub(super) static FS_FUNCTIONS: &[BuiltIn] = &[
    BuiltIn {
        name: "read",
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::String),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsRead),
        ..
    },
    BuiltIn {
        name: "exists",
        positional: &[&[SignatureType::String]],
        return_type: ReturnType::Type(SignatureType::Bool),
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsExists),
        ..
    },
    BuiltIn {
        name: "write",
        positional: &[&[SignatureType::String], &[SignatureType::String]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsWrite),
        ..
    },
    BuiltIn {
        name: "append",
        positional: &[&[SignatureType::String], &[SignatureType::String]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::TwoOperands(Instr::FsAppend),
        ..
    },
    BuiltIn {
        name: "delete",
        positional: &[&[SignatureType::String]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::OneOperand(Instr::FsDelete),
        ..
    },
    BuiltIn {
        name: "delete_dir",
        positional: &[&[SignatureType::String]],
        error_span: ErrorSpan::Callsite,
        lowering: Lowering::OneOperand(Instr::FsDeleteDir),
        ..
    },
];
