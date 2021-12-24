// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::*;
use crate::externs::TestExterns;
use crate::rand::ReplayingRand;
use blockstore::{Blockstore, MemoryBlockstore};
use cid::Cid;
use delegate::delegate;
use fvm::externs::{Externs, Rand};
use fvm::kernel::{
    ActorOps, BlockId, BlockOps, BlockStat, CircSupplyOps, CryptoOps, DebugOps, GasOps, MessageOps,
    NetworkOps, RandomnessOps, SelfOps, SendOps, SyscallError, ValidationOps,
};
use fvm::{DefaultKernel, Kernel};
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::Randomness;
use fvm_shared::receipt::Receipt;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum};
use std::collections::HashMap;

/// A test kernel is backed by a real kernel (DefaultKernel), which in turn uses
/// a MemoryBlockstore and the TestExterns. This kernel patches:
/// - some crypto operations to return fixed values, as required by the test
///   vectors.
/// - the circulating supply syscall, to return a fixed TokenAmount, determined
///   by the test vector.    
pub struct TestKernel<'a> {
    default: DefaultKernel<MemoryBlockstore, TestExterns<'a>>,
    circ_supply: TokenAmount,
}

impl<'a> ActorOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn resolve_address(&self, address: &Address) -> fvm::kernel::Result<Option<ActorID>>;
            fn get_actor_code_cid(&self, addr: &Address) -> fvm::kernel::Result<Option<Cid>>;
            fn new_actor_address(&mut self) -> fvm::kernel::Result<Address>;
            fn create_actor(&mut self, code_id: Cid, address: &Address) -> fvm::kernel::Result<()>;
        }
    }
}

impl<'a> BlockOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn block_open(&mut self, cid: &Cid) -> fvm::kernel::Result<BlockId>;
            fn block_create(&mut self, codec: u64, data: &[u8]) -> fvm::kernel::Result<BlockId>;
            fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid>;
            fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> fvm::kernel::Result<u32>;
            fn block_stat(&self, id: BlockId) -> fvm::kernel::Result<BlockStat>;
        }
    }
}

impl<'a> CircSupplyOps for TestKernel<'a> {
    fn total_fil_circ_supply(&self) -> fvm::kernel::Result<TokenAmount> {
        Ok(self.circ_supply.clone())
    }
}

impl<'a> CryptoOps for TestKernel<'a> {
    delegate! {
        // Delegate to the real thing.
        to self.default {
            fn hash_blake2b(&mut self, data: &[u8]) -> fvm::kernel::Result<[u8; 32]>;

            fn compute_unsealed_sector_cid(
                &mut self,
                proof_type: RegisteredSealProof,
                infos: &[PieceInfo],
            ) -> fvm::kernel::Result<Cid>;

            fn batch_verify_seals(
                &mut self,
                infos: &[(&Address, &[SealVerifyInfo])],
            ) -> fvm::kernel::Result<HashMap<Address, Vec<bool>>>;
        }

        fn verify_signature(&self, _: &Signature, _: &Address, _: &[u8]) -> fvm::kernel::Result<()> {
            Ok(())
        }

        fn verify_seal(&self, _: &SealVerifyInfo) -> fvm::kernel::Result<()> {
            Ok(())
        }
        fn verify_post(&self, _: &WindowPoStVerifyInfo) -> fvm::kernel::Result<bool> {
            Ok(true)
        }

        // TODO check if this should be defaulted as well
        fn verify_consensus_fault(
            &self,
            _: &[u8],
            _: &[u8],
            _: &[u8],
        ) -> fvm::kernel::Result<Option<ConsensusFault>> {
            Ok(None)
        }

        fn verify_aggregate_seals(
            &self,
            _: &fil_types::AggregateSealVerifyProofAndInfos,
        ) -> fvm::kernel::Result<()> {
            Ok(())
        }
    }
}

impl<'a> DebugOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn push_syscall_error(&mut self, e: SyscallError);
            fn push_actor_error(&mut self, code: ExitCode, message: String);
            fn clear_error(&mut self);
        }
    }
}

impl<'a> GasOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn charge_gas(&mut self, name: &str, compute: i64) -> fvm::kernel::Result<()>;
        }
    }
}

impl<'a> MessageOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn msg_caller(&self) -> ActorID;
            fn msg_receiver(&self) -> ActorID;
            fn msg_method_number(&self) -> MethodNum;
            fn msg_value_received(&self) -> TokenAmount;
        }
    }
}

impl<'a> NetworkOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn network_epoch(&self) -> ChainEpoch;
            fn network_version(&self) -> NetworkVersion;
            fn network_base_fee(&self) -> &TokenAmount;
        }
    }
}

impl<'a> RandomnessOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn get_randomness_from_tickets(
                &self,
                personalization: DomainSeparationTag,
                rand_epoch: ChainEpoch,
                entropy: &[u8],
            ) -> fvm::kernel::Result<Randomness>;

            fn get_randomness_from_beacon(
                &self,
                personalization: DomainSeparationTag,
                rand_epoch: ChainEpoch,
                entropy: &[u8],
            ) -> fvm::kernel::Result<Randomness>;
        }
    }
}

impl<'a> SelfOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn root(&self) -> Cid;
            fn set_root(&mut self, root: Cid) -> fvm::kernel::Result<()>;
            fn current_balance(&self) -> fvm::kernel::Result<TokenAmount>;
            fn self_destruct(&mut self, beneficiary: &Address) -> fvm::kernel::Result<()>;
        }
    }
}

impl<'a> SendOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn send(
                &mut self,
                recipient: &Address,
                method: u64,
                params: &RawBytes,
                value: &TokenAmount,
            ) -> fvm::kernel::Result<Receipt>;
        }
    }
}

impl<'a> ValidationOps for TestKernel<'a> {
    delegate! {
        to self.default {
            fn validate_immediate_caller_accept_any(&mut self) -> fvm::kernel::Result<()>;
            fn validate_immediate_caller_addr_one_of(
                &mut self,
                allowed: &[Address],
            ) -> fvm::kernel::Result<()>;
            fn validate_immediate_caller_type_one_of(
                &mut self,
                allowed: &[Cid],
            ) -> fvm::kernel::Result<()>;
        }
    }
}

impl<'a> Kernel for TestKernel<'a> {
    type Blockstore = MemoryBlockstore;
    type Externs = TestExterns<'a>;

    fn new(
        mgr: fvm::call_manager::CallManager<Self>,
        from: ActorID,
        to: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
    ) -> Self
    where
        Self: Sized,
    {
        Self(DefaultKernel::new(mgr, from, to, method, value_received))
    }

    fn take(self) -> fvm::call_manager::CallManager<Self>
    where
        Self: Sized,
    {
        self.default.take()
    }
}
