use std::collections::VecDeque;

use anyhow::Result;
#[allow(unused_imports)]
use wasmtime::{Config as WasmtimeConfig, Engine, Instance, Linker, Module, Store};

use blockstore::Blockstore;
use fvm_shared::actor_error;
use fvm_shared::address::Address;
use fvm_shared::error::ActorError;

use crate::externs::Externs;
use crate::gas::GasTracker;
use crate::machine::{CallStack, Machine, MachineContext};
use crate::message::Message;
use crate::state_tree::ActorState;
use crate::{DefaultKernel, Kernel};

/// An entry in the return stack.
type ReturnEntry = (bool, Vec<u8>);

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
    pub fn run<'a, 'db, B, E, K>(
        machine: &'a Machine<B, E, K>,
        call_stack: &'a CallStack<'a, 'db, B>,
        msg: &'a Message,
        bytecode: &[u8],
    ) -> anyhow::Result<()>
    where
        B: Blockstore,
        E: Externs,
        K: Kernel,
        'db: 'a,
    {
        let engine = machine.engine();
        let module = Module::new(engine, bytecode)?;
        let kernel = DefaultKernel::create(machine, call_stack, msg.clone())?;
        let mut store = Store::new(engine, kernel);
        let instance = machine.linker().instantiate(store, &module)?;

        let invoke = instance.get_typed_func(&mut store, "invoke")?;
        let (result,): (u32,) = invoke.call(&mut store, (method_params,))?;
        println!("{:?}", result);
        Ok(())

        //

        todo!()
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

    /// Describes the top element in the return stack.
    /// -1 means error, 0 means non-existent, otherwise the length is returned.
    pub fn return_desc(&self) -> i64 {
        self.return_stack.back().map_or(0, |e| {
            if !e.0 {
                return -1;
            }
            e.1.len() as i64
        })
    }

    pub fn return_discard(&mut self) {
        self.return_stack.pop_back();
    }

    /// Copies the top of the stack into
    pub fn return_pop(&mut self, into: &[u8]) {
        self.return_stack.pop_back();
    }
}
