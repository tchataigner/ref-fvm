use std::borrow::Borrow;
use std::convert::{TryFrom, TryInto};
use std::error::Error;

use anyhow::{anyhow, Result};
use cid::Cid;

use blockstore::Blockstore;
use fvm_shared::ActorID;

use crate::externs::Externs;
use crate::machine::{CallStack, Machine};
use crate::message::Message;

use super::blocks::{Block, BlockRegistry};
use super::*;

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<'a, 'db, B, E> {
    /// The machine this kernel is bound to.
    machine: &'a Machine<'db, B, E, Self>,
    /// The call stack in which the invocation container to which this kernel
    /// is bound is participating in.
    call_stack: &'a CallStack<'a, 'db, B>,
    /// The message being processed by the invocation container to which this
    /// kernel is bound.
    ///
    /// Owned copy.
    invocation_msg: Message,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
    /// Blockstore cloned from the machine.
    blockstore: &'db B,
}

// Even though all children traits are implemented, Rust needs to know that the
// supertrait is implemented too.
impl<B, E> Kernel for DefaultKernel<'_, '_, B, E>
where
    B: Blockstore,
    E: Externs,
{
}

impl<'a, 'db, B, E> DefaultKernel<'a, 'db, B, E>
where
    B: Blockstore,
    E: Externs,
    'db: 'a,
{
    pub fn create(
        machine: &'a Machine<'db, B, E, Self>,
        call_stack: &'a CallStack<'a, 'db, B>,
        mut invocation_msg: Message,
    ) -> Result<Self, Box<dyn Error>> {
        invocation_msg.from = call_stack
            .state_tree()
            .lookup_id(&invocation_msg.from)?
            .ok_or("failed to lookup from id address")?;

        invocation_msg.to = call_stack
            .state_tree()
            .lookup_id(&invocation_msg.to)?
            .ok_or("failed to lookup to id address")?;

        Ok(DefaultKernel {
            invocation_msg,
            call_stack,
            machine,
            blocks: BlockRegistry::new(),
            blockstore: machine.blockstore(),
        })
    }
}

impl<B, E> ActorOps for DefaultKernel<'_, '_, B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn root(&self) -> &Cid {
        let addr = &self.invocation_msg.to;
        let state = self
            .call_stack
            .state_tree()
            .get_actor(addr)
            .unwrap()
            .expect("expected invoked actor to exist");
        &state.state
    }

    fn set_root(&mut self, new: Cid) -> anyhow::Result<()> {
        let state_tree = self.call_stack.state_tree_mut();
        state_tree
            .mutate_actor(&self.invocation_msg.to, |actor_state| {
                actor_state.state = new;
                Ok(())
            })
            .map_err(|e| anyhow!(e.to_string()))
    }
}

impl<B, E> BlockOps for DefaultKernel<'_, '_, B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, BlockError> {
        let data = self
            .blockstore
            .get(cid)
            .map_err(|e| BlockError::Internal(e.into()))?
            .ok_or_else(|| BlockError::MissingState(Box::new(*cid)))?;

        let block = Block::new(cid.codec(), data);
        self.blocks.put(block)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, BlockError> {
        self.blocks.put(Block::new(codec, data))
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, BlockError> {
        use multihash::MultihashDigest;
        let block = self.blocks.get(id)?;
        let code =
            multihash::Code::try_from(hash_fun)
                .ok()
                .ok_or(BlockError::InvalidMultihashSpec {
                    code: hash_fun,
                    length: hash_len,
                })?;

        let hash = code.digest(&block.data());
        if u32::from(hash.size()) < hash_len {
            return Err(BlockError::InvalidMultihashSpec {
                code: hash_fun,
                length: hash_len,
            });
        }
        let k = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));
        // TODO: for now, we _put_ the block here. In the future, we should put it into a write
        // cache, then flush it later.
        self.blockstore
            .put(&k, block.data())
            .map_err(|e| BlockError::Internal(Box::new(e)))?;
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, BlockError> {
        let data = &self.blocks.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        self.blocks.get(id).map(|b| BlockStat {
            codec: b.codec(),
            size: b.size(),
        })
    }
}

impl<B, E> InvocationOps for DefaultKernel<'_, '_, B, E>
where
    B: Blockstore,
    E: Externs,
{
    fn method_number(&self) -> MethodId {
        self.invocation_msg.method_num
    }

    fn method_params(&self) -> BlockId {
        // TODO
        0
    }

    fn caller(&self) -> ActorID {
        self.invocation_msg
            .from
            .id()
            .expect("invocation from address was not an ID address")
    }

    fn receiver(&self) -> ActorID {
        self.invocation_msg
            .to
            .id()
            .expect("invocation to address was not an ID address")
    }

    fn value_received(&self) -> u128 {
        // TODO @steb
        // self.invocation_msg.value.into()
        0
    }
}
