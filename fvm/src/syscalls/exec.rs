use crate::kernel::ExecutionError;
use crate::syscalls::context::Context;
use crate::Kernel;
use fvm_shared::error::{ActorError, ExitCode};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use wasmtime::{Caller, Trap};

/// Raises an actor-driven abort.
pub fn abort(
    caller: Caller<'_, impl Kernel>,
    exit_code: u64,
    msg_off: u32,
    msg_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let msg = ctx.try_slice(msg_off, msg_len)?;
    // TODO this will currently panic if the value is out of bounds of the enum
    //  this is fine because built-in actors use the enum; but we need to define
    //  a standard behaviour for illegal exit codes reported by user-defined actors
    let code: ExitCode = FromPrimitive::from_u64(exit_code).unwrap();
    // TODO assuming abort messages passed through the syscall boundary will be UTF-8,
    //  and refusing to process unsafe messages.
    let msg = String::from_utf8(msg.to_owned()).unwrap_or("non-utf8 abort message".to_owned());
    let err: Box<dyn std::error::Error + Send + Sync> =
        Box::new(ExecutionError::Actor(ActorError::new(code, msg)));
    Err(err.into())
}
