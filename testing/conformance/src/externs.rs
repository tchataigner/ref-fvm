use crate::rand::ReplayingRand;
use blockstore::Blockstore;
use cid::Cid;
use delegate::delegate;
use fvm::externs::{Consensus, Externs, Rand};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;

/// TestExterns forward randomness requests to ReplayingRand.
pub struct TestExterns<'a> {
    rand: ReplayingRand<'a>,
}

impl Externs for TestExterns {}

impl Rand for TestExterns {
    delegate! {
        to self.rand {
            fn get_chain_randomness(
                &self,
                pers: DomainSeparationTag,
                round: ChainEpoch,
                entropy: &[u8],
            ) -> anyhow::Result<[u8; 32]>;

            fn get_chain_randomness_looking_forward(
                &self,
                pers: DomainSeparationTag,
                round: ChainEpoch,
                entropy: &[u8],
            ) -> anyhow::Result<[u8; 32]>;

            fn get_beacon_randomness(
                &self,
                pers: DomainSeparationTag,
                round: ChainEpoch,
                entropy: &[u8],
            ) -> anyhow::Result<[u8; 32]>;

            fn get_beacon_randomness_looking_forward(
                &self,
                pers: DomainSeparationTag,
                round: ChainEpoch,
                entropy: &[u8],
            ) -> anyhow::Result<[u8; 32]>;
        }
    }
}

impl Consensus for TestExterns {
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<Option<ConsensusFault>> {
        todo!()
    }
}

impl Blockstore for TestExterns {
    type Error = ();

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        todo!()
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }
}
