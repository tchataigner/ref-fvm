use cid::Cid;
use fvm::externs::{Consensus, Rand};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use std::convert::Infallible;

pub struct DummyExterns;

impl Rand for DummyExterns {
    fn get_chain_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        todo!()
    }

    fn get_chain_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        todo!()
    }

    fn get_beacon_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        todo!()
    }

    fn get_beacon_randomness_looking_forward(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        todo!()
    }
}

impl Consensus for DummyExterns {
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<ConsensusFault> {
        todo!()
    }
}

impl blockstore::Blockstore for DummyExterns {
    type Error = Infallible;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        todo!()
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        todo!()
    }

    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }

    fn delete(&self, k: &Cid) -> Result<(), Self::Error> {
        todo!()
    }
}

impl fvm::externs::Externs for DummyExterns {}
