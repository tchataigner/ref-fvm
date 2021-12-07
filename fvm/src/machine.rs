use actor::ActorDowncast;
use anyhow::anyhow;
use cid::Cid;
use num_traits::Zero;
use std::marker::PhantomData;
use wasmtime::{Engine, Linker};

use blockstore::Blockstore;
use fvm_shared::actor_error;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::error::{ActorError, ExitCode};

use crate::externs::Externs;
use crate::gas::{price_list_by_epoch, GasTracker, PriceList};
use crate::invocation::InvocationContainer;
use crate::kernel::Kernel;
use crate::message::Message;
use crate::receipt::Receipt;
use crate::state_tree::{ActorState, StateTree};
use crate::syscalls::bind_syscalls;
use crate::Config;

/// The core of the FVM.
///
/// ## Generic types
/// * B => Blockstore.
/// * E => Externs.
/// * K => Kernel.
pub struct Machine<'a, 'db, B, E, K> {
    config: Config,
    /// The context for the execution.
    context: MachineContext,
    /// The wasmtime engine is created on construction of the Machine, and
    /// is dropped when the Machine is dropped.
    engine: Engine,
    /// The linker used to store wasm functions.
    linker: Linker<K>,
    /// Blockstore to use for this machine instance.
    blockstore: &'db B,
    /// Boundary A calls are handled through externs. These are calls from the
    /// FVM to the Filecoin node.
    externs: E,
    /// The state tree. It is updated with the results from every message
    /// execution as the call stack for every message concludes.
    ///
    /// Owned.
    state_tree: StateTree<'db, B>,
    /// The buffer of blocks to be committed to the blockstore after
    /// execution concludes.
    /// TODO @steb needs to figure out how all of this is going to work.
    commit_buffer: (),
    // Placeholder to maybe keep a reference to FullVerifier (Forest) here.
    // The FullVerifier is the gateway to filecoin-proofs-api.
    // TODO these likely go in the kernel, as they are syscalls that can be
    // resolved inside the FVM without traversing Boundary A.
    // verifier: PhantomData<V>,
    // The currently active call stack.
    // TODO I don't think we need to store this in the state; it can probably
    // be a stack variable in execute_message.
    // @steb says we _can't_ store this state.
    // call_stack: CallStack<'db, B>,
    phantom: &'a PhantomData<()>,
}

impl<'a, 'db, B, E, K: 'static> Machine<'a, 'db, B, E, K>
where
    B: Blockstore,
    E: Externs,
    K: Kernel,
{
    pub fn new(
        config: Config,
        epoch: ChainEpoch,
        base_fee: &TokenAmount,
        state_root: &Cid,
        blockstore: &'db B,
        externs: E,
    ) -> anyhow::Result<Machine<'a, 'db, B, E, K>> {
        let context = MachineContext::new(
            epoch,
            base_fee.clone(),
            state_root.clone(),
            price_list_by_epoch(epoch),
        );

        // Initialize the WASM engine.
        let engine = Engine::new(&config.engine)?;
        let mut linker = Linker::new(&engine);
        // TODO turn into a trait so we can do Linker::new(&engine).with_bound_syscalls();
        bind_syscalls(&mut linker)?;

        // TODO: fix the error handling to use anyhow up and down the stack, or at least not use
        // non-send errors in the state-tree.
        let state_tree = StateTree::new_from_root(blockstore, &context.state_root)
            .map_err(|e| anyhow!(e.to_string()))?;

        Ok(Machine {
            config,
            linker,
            context,
            engine,
            externs,
            blockstore,
            state_tree,
            commit_buffer: Default::default(), // @stebalien TBD
            phantom: &Default::default(),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn linker(&self) -> &Linker<K> {
        &self.linker
    }

    pub fn config(&self) -> Config {
        self.config.clone()
    }

    pub fn blockstore(&self) -> &'db B {
        self.blockstore.clone()
    }

    /// This is the entrypoint to execute a message.
    pub fn execute_message(&mut self, msg: &Message, kind: ApplyKind) -> anyhow::Result<ApplyRet> {
        // TODO sanity check on message, copied from Forest, needs adaptation.
        msg.check()?;

        // TODO I don't like having price lists _inside_ the FVM, but passing
        //  these across the boundary is also a no-go.
        let pl = &self.context.price_list;
        let ser_msg = msg.marshal_cbor()?;
        let msg_gas_cost = pl.on_chain_message(ser_msg.len());
        let cost_total = msg_gas_cost.total();

        // Verify the cost of the message is not over the message gas limit.
        // TODO handle errors properly
        if cost_total > msg.gas_limit {
            let err =
                actor_error!(SysErrOutOfGas; "Out of gas ({} > {})", cost_total, msg.gas_limit);
            return Ok(ApplyRet::prevalidation_fail(
                ExitCode::SysErrOutOfGas,
                &self.context.base_fee * cost_total,
                Some(err),
            ));
        }

        // Load sender actor state.
        let miner_penalty_amount = &self.context.base_fee * msg.gas_limit;
        let sender = match self.state_tree.get_actor(&msg.from) {
            Ok(Some(sender)) => sender,
            _ => {
                return Ok(ApplyRet {
                    msg_receipt: Receipt {
                        return_data: RawBytes::default(),
                        exit_code: ExitCode::SysErrSenderInvalid,
                        gas_used: 0,
                    },
                    penalty: miner_penalty_amount,
                    act_error: Some(actor_error!(SysErrSenderInvalid; "Sender invalid")),
                    miner_tip: BigInt::zero(),
                });
            }
        };

        // If sender is not an account actor, the message is invalid.
        if !actor::is_account_actor(&sender.code) {
            return Ok(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(actor_error!(SysErrSenderInvalid; "send not from account actor")),
                miner_tip: BigInt::zero(),
            });
        };

        // Check sequence is correct
        if msg.sequence != sender.sequence {
            return Ok(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderStateInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(actor_error!(SysErrSenderStateInvalid;
                    "actor sequence invalid: {} != {}", msg.sequence, sender.sequence)),
                miner_tip: BigInt::zero(),
            });
        };

        // Ensure from actor has enough balance to cover the gas cost of the message.
        let gas_cost: TokenAmount = msg.gas_fee_cap.clone() * msg.gas_limit.clone();
        if sender.balance < gas_cost {
            return Ok(ApplyRet {
                msg_receipt: Receipt {
                    return_data: RawBytes::default(),
                    exit_code: ExitCode::SysErrSenderStateInvalid,
                    gas_used: 0,
                },
                penalty: miner_penalty_amount,
                act_error: Some(actor_error!(SysErrSenderStateInvalid;
                    "actor balance less than needed: {} < {}", sender.balance, gas_cost)),
                miner_tip: BigInt::zero(),
            });
        };

        // Deduct gas cost and increment sequence
        self.state_tree
            .mutate_actor(&msg.from, |act| {
                act.deduct_funds(&gas_cost)?;
                act.sequence += 1;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))?;

        self.state_tree.snapshot().map_err(anyhow::Error::msg)?;

        // initial gas cost is the message inclusion gas.
        let mut gas_tracker = GasTracker::new(msg.gas_limit, msg_gas_cost.total());

        // TODO error handling
        self.state_tree.snapshot().unwrap();

        CallStack::perform(msg, &self.context, &mut self.state_tree, &mut gas_tracker);

        // let ic = InvocationContainer{
        //     kernel: &self.kernel,
        //     machine_context: &self.context,
        //     gas_tracker: &gas_tracker,
        //     actor_bytecode: &[],
        //     instance: &(),
        //     return_stack: Default::default()
        // };
        //

        // Perform state transition
        // // TODO: here is where we start the call stack and the invocation container.
        // let (mut ret_data, rt, mut act_err) = self.send(msg.message(), Some(msg_gas_cost));
        // if let Some(err) = &act_err {
        //     if err.is_fatal() {
        //         return Err(format!(
        //             "[from={}, to={}, seq={}, m={}, h={}] fatal error: {}",
        //             msg.from(),
        //             msg.to(),
        //             msg.sequence(),
        //             msg.method_num(),
        //             self.epoch,
        //             err
        //         ));
        //     } else {
        //         debug!(
        //             "[from={}, to={}, seq={}, m={}] send error: {}",
        //             msg.from(),
        //             msg.to(),
        //             msg.sequence(),
        //             msg.method_num(),
        //             err
        //         );
        //         if !ret_data.is_empty() {
        //             return Err(format!(
        //                 "message invocation errored, but had a return value anyway: {}",
        //                 err
        //             ));
        //         }
        //     }
        // }

        // let gas_used = if let Some(mut rt) = rt {
        //     if !ret_data.is_empty() {
        //         if let Err(e) = rt.charge_gas(rt.price_list().on_chain_return_value(ret_data.len()))
        //         {
        //             act_err = Some(e);
        //             ret_data = Serialized::default();
        //         }
        //     }
        //     if rt.gas_used() < 0 {
        //         0
        //     } else {
        //         rt.gas_used()
        //     }
        // } else {
        //     return Err(format!("send returned None runtime: {:?}", act_err));
        // };
        //
        // let err_code = if let Some(err) = &act_err {
        //     if !err.is_ok() {
        //         // Revert all state changes on error.
        //         self.state.revert_to_snapshot()?;
        //     }
        //     err.exit_code()
        // } else {
        //     ExitCode::Ok
        // };
        //
        // let should_burn = self
        //     .should_burn(self.state(), msg, err_code)
        //     .map_err(|e| format!("failed to decide whether to burn: {}", e))?;
        //
        // let GasOutputs {
        //     base_fee_burn,
        //     miner_tip,
        //     over_estimation_burn,
        //     refund,
        //     miner_penalty,
        //     ..
        // } = compute_gas_outputs(
        //     gas_used,
        //     msg.gas_limit(),
        //     &self.base_fee,
        //     msg.gas_fee_cap(),
        //     msg.gas_premium().clone(),
        //     should_burn,
        // );
        //
        // let mut transfer_to_actor = |addr: &Address, amt: &TokenAmount| -> Result<(), String> {
        //     if amt.sign() == Sign::Minus {
        //         return Err("attempted to transfer negative value into actor".into());
        //     }
        //     if amt.is_zero() {
        //         return Ok(());
        //     }
        //
        //     self.state
        //         .mutate_actor(addr, |act| {
        //             act.deposit_funds(amt);
        //             Ok(())
        //         })
        //         .map_err(|e| e.to_string())?;
        //     Ok(())
        // };
        //
        // transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &base_fee_burn)?;
        //
        // transfer_to_actor(&**reward::ADDRESS, &miner_tip)?;
        //
        // transfer_to_actor(&*BURNT_FUNDS_ACTOR_ADDR, &over_estimation_burn)?;
        //
        // // refund unused gas
        // transfer_to_actor(msg.from(), &refund)?;
        //
        // if &base_fee_burn + over_estimation_burn + &refund + &miner_tip != gas_cost {
        //     // Sanity check. This could be a fatal error.
        //     return Err("Gas handling math is wrong".to_owned());
        // }
        // self.state.clear_snapshot()?;
        //
        // Ok(ApplyRet {
        //     msg_receipt: MessageReceipt {
        //         return_data: ret_data,
        //         exit_code: err_code,
        //         gas_used,
        //     },
        //     penalty: miner_penalty,
        //     act_error: act_err,
        //     miner_tip,
        // })

        // TODO once the CallStack finishes running, copy over the resulting state tree layer to the Machine's state tree
        // TODO pull the receipt from the CallStack and return it.
        // Ok(Default::default())
        todo!("return the receipt")
    }
}

/// Apply message return data.
#[derive(Clone, Debug)]
pub struct ApplyRet {
    /// Message receipt for the transaction. This data is stored on chain.
    pub msg_receipt: Receipt,
    /// Actor error from the transaction, if one exists.
    pub act_error: Option<ActorError>,
    /// Gas penalty from transaction, if any.
    pub penalty: BigInt,
    /// Tip given to miner from message.
    pub miner_tip: BigInt,
}

impl ApplyRet {
    #[inline]
    pub fn prevalidation_fail(
        exit_code: ExitCode,
        miner_penalty: BigInt,
        error: Option<ActorError>,
    ) -> ApplyRet {
        ApplyRet {
            msg_receipt: Receipt {
                exit_code,
                return_data: RawBytes::default(),
                gas_used: 0,
            },
            penalty: miner_penalty,
            act_error: error,
            miner_tip: BigInt::zero(),
        }
    }
}

pub struct CallStack<'a, 'db, B> {
    /// The buffer of blocks that that a given message execution has written.
    /// Reachable blocks from the updated state roots of actors touched by the
    /// call stack will probably need to be transferred to the Machine's
    /// commit_buffer.
    /// TODO @steb needs to figure out how all of this is going to work.
    // write_buffer: (),
    /// A state tree stacked on top of the Machine state tree, tracking state
    /// changes performed by actors throughout a call stack.
    state_tree: &'a mut StateTree<'db, B>,
    // TODO figure out what else needs to be here.
    /// The original message that spawned the call stack.
    orig_msg: &'a Message,
    /// The gas tracker for the transaction.
    gas_tracker: &'a mut GasTracker,
    machine_context: &'a MachineContext,
}

impl<'a, 'db, B> CallStack<'a, 'db, B>
where
    B: Blockstore,
{
    fn perform(
        msg: &'a Message,
        machine_context: &'a MachineContext,
        state_tree: &'a mut StateTree<'db, B>,
        gas_tracker: &'a mut GasTracker,
    ) -> anyhow::Result<Receipt> {
        let mut call_stack = CallStack {
            state_tree,
            gas_tracker,
            machine_context,
            orig_msg: msg,
        };
        call_stack.call_next(msg)
    }

    pub fn state_tree(&self) -> &StateTree<'db, B> {
        self.state_tree
    }

    pub fn state_tree_mut(&self) -> &mut StateTree<'db, B> {
        // This is safe only because the VM is single-threaded at this stage.
        self.state_tree
    }

    pub fn call_next(&mut self, msg: &Message) -> anyhow::Result<Receipt> {
        // Clone because we may override the receiver in the message.
        let mut msg = msg.clone();

        // Get the receiver; this will resolve the address.
        let receiver = match self
            .state_tree
            .lookup_id(&msg.to)
            .map_err(|e| anyhow::Error::msg(e.to_string()))?
        {
            Some(addr) => addr,
            None => match msg.to.protocol() {
                Protocol::BLS | Protocol::Secp256k1 => {
                    // Try to create an account actor if the receiver is a key address.
                    let (_, id_addr) = self.try_create_account_actor(&msg.to)?;
                    msg.to = id_addr;
                    id_addr
                }
                _ => return Err(anyhow!("actor not found: {}", msg.to)),
            },
        };

        // TODO Load the code for the receiver by CID (state.code).
        // TODO The node's blockstore will need to return the appropriate WASM
        //  code for built-in system actors. Either we implement a load_code(cid)
        //  Boundary A syscall, or a special blockstore with static mappings from
        //  CodeCID => WASM bytecode for built-in actors will be necessary on the
        //  node side.

        // TODO instantiate a WASM instance, wrapping the InvocationContainer as
        //  the store data.

        // TODO invoke the entrypoint on the WASM instance.

        // TODO somehow instrument so that sends are looped into the call stack.

        todo!()
    }

    pub fn try_create_account_actor(
        &mut self,
        addr: &Address,
    ) -> Result<(ActorState, Address), ActorError> {
        self.gas_tracker
            .charge_gas(self.machine_context.price_list.on_create_actor())?;

        if addr.is_bls_zero_address() {
            actor_error!(SysErrIllegalArgument; "cannot create the bls zero address actor");
        }

        let addr_id = self
            .state_tree
            .register_new_address(addr)
            .map_err(|e| e.downcast_fatal("failed to register new address"))?;

        let act = crate::account_actor::ZERO_STATE.clone();

        self.state_tree
            .set_actor(&addr_id, act)
            .map_err(|e| e.downcast_fatal("failed to set actor"))?;

        let params = RawBytes::serialize(&addr).map_err(|e| {
            actor_error!(fatal(
                "couldn't serialize params for actor construction: {:?}",
                e
            ))
        })?;

        let msg = Message {
            from: *crate::account_actor::SYSTEM_ACTOR_ADDR,
            to: addr.clone(),
            method_num: fvm_shared::METHOD_CONSTRUCTOR,
            value: TokenAmount::from(0_u32),
            params,
            gas_limit: self.gas_tracker.gas_available(),
            version: Default::default(),
            sequence: Default::default(),
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        };

        /// TODO handle error properly
        self.call_next(&msg).map_err(|e| actor_error!(fatal(e)))?;

        let act = self
            .state_tree
            .get_actor(&addr_id)
            .map_err(|e| e.downcast_fatal("failed to get actor"))?
            .ok_or_else(|| actor_error!(fatal("failed to retrieve created actor state")))?;

        Ok((act, addr_id))
    }

    // TODO need accessors to check the outcome, and merge this state tree onto
    // the machine's state tree.
}

pub enum ApplyKind {
    Explicit,
    Implicit,
}

/// Execution context supplied to the machine. All fields are private.
/// Epoch and base fee cannot be mutated. The state_root corresponds to the
/// initial state root, and gets updated internally with every message execution.
pub struct MachineContext {
    /// The epoch at which the Machine runs.
    epoch: ChainEpoch,
    /// The base fee that's in effect when the Machine runs.
    base_fee: TokenAmount,
    state_root: Cid,
    price_list: PriceList,
}

impl MachineContext {
    fn new(
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        state_root: Cid,
        price_list: PriceList,
    ) -> MachineContext {
        MachineContext {
            epoch,
            base_fee,
            state_root,
            price_list,
        }
    }

    pub fn epoch(self) -> ChainEpoch {
        self.epoch
    }

    pub fn base_fee(&self) -> &TokenAmount {
        &self.base_fee
    }

    pub fn state_root(&self) -> &Cid {
        &self.state_root
    }

    pub fn price_list(&self) -> &PriceList {
        &self.price_list
    }

    pub fn set_state_root(&mut self, state_root: Cid) {
        self.state_root = state_root
    }
}
