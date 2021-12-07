use std::collections::VecDeque;

use anyhow::{anyhow, Result};
use multihash::Code::Blake2b256;
#[allow(unused_imports)]
use wasmtime::{Config as WasmtimeConfig, Engine, Instance, Linker, Module, Store};

use blockstore::Blockstore;
use fvm_shared::actor_error;
use fvm_shared::address::Address;
use fvm_shared::encoding::DAG_CBOR;
use fvm_shared::error::ActorError;

use crate::externs::Externs;
use crate::gas::GasTracker;
use crate::kernel::BlockOps;
use crate::machine::{CallStack, Machine, MachineContext};
use crate::message::Message;
use crate::state_tree::ActorState;
use crate::{DefaultKernel, Kernel};

/// The InvocationContainer is the store data associated with a
/// wasmtime instance.
pub struct InvocationContainer {}

/// TODO it's possible that the invocation container doesn't need to exist
/// as an object; instead the invocation container could be the "store data"
/// inside the wasmtime store. If so, the CallStack would instantiate the
/// wasmtime::Instance and wire in the store data.
///
/// Although having said that, that solution is entirely wasmtime specific, and
/// will lock us right into that runtime. We probably _should_ have an
/// InvocationContainer to abstract underlying WASM runtime implementation
/// details.
impl InvocationContainer {
    pub fn run<'a, 'db, B, E>(
        machine: &'a Machine<'a, 'db, B, E, DefaultKernel<'_, 'db, B, E>>,
        call_stack: &'a CallStack<'a, 'db, B>,
        msg: &'a Message,
        bytecode: &[u8],
    ) -> anyhow::Result<()>
    where
        B: Blockstore,
        E: Externs,
        'db: 'a,
    {
        let engine = machine.engine();
        let module = Module::new(engine, bytecode)?;
        let mut kernel = DefaultKernel::create(machine, call_stack, msg.clone())
            .map_err(|e| anyhow!(e.to_string()))?;
        let mut store = Store::new(engine, kernel);
        let instance = machine.linker().instantiate(store, &module)?;

        // Inject the message parameters as a block in the block registry.
        let params_block_id = kernel.block_create(DAG_CBOR, msg.params.bytes())?;

        let invoke = instance.get_typed_func(&mut store, "invoke")?;
        let (result,): (u32,) = invoke.call(&mut store, (params_block_id))?;
        println!("{:?}", result);
        Ok(())
    }

    // TODO
    // pub fn handle(msg: Message) {
    //     // Get the callee; this will resolve the address.
    //     // TODO it's not clear to me reading Forest's VM what should happen here
    //     //  There, this happens in the internal_send.
    //     let callee = match self.state_tree.get_actor(&msg.to) {
    //         Ok(addr) => ,
    //         Err(e) => Ok(ApplyRet::prevalidation_fail()),
    //     };
    //     let to_actor = match self
    //         .state
    //         .get_actor(msg.to())
    //         .map_err(|e| e.downcast_fatal("failed to get actor"))?
    //     {
    //         Some(act) => act,
    //         None => {
    //             // Try to create actor if not exist
    //             let (to_actor, id_addr) = self.try_create_account_actor(msg.to())?;
    //             if self.network_version() > NetworkVersion::V3 {
    //                 // Update the receiver to the created ID address
    //                 self.vm_msg.receiver = id_addr;
    //             }
    //             to_actor
    //         }
    //     };
    // }
}
