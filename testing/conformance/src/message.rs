// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use blockstore::Blockstore;
use fvm::Config;
use fvm::externs::Externs;
use fvm::machine::ApplyRet;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::version::NetworkVersion;
use crate::rand::ReplayingRand;
use crate::vector::Selector;
use super::*;

#[derive(Debug, Deserialize)]
pub struct MessageVector {
    #[serde(with = "base64_bytes")]
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub epoch_offset: Option<ChainEpoch>,
}

pub struct ExecuteMessageParams<'a> {
    pub pre_root: &'a Cid,
    pub epoch: ChainEpoch,
    pub msg: &'a Message,
    pub circ_supply: TokenAmount,
    pub basefee: TokenAmount,
    pub randomness: ReplayingRand<'a>,
    pub network_version: NetworkVersion,
}

struct MockCircSupply(TokenAmount);
impl Circ for MockCircSupply {
    fn get_supply<DB: BlockStore>(
        &self,
        _: ChainEpoch,
        _: &StateTree<DB>,
    ) -> Result<TokenAmount, Box<dyn StdError>> {
        Ok(self.0.clone())
    }
}

// struct MockStateLB<'db, MemoryDB>(&'db MemoryDB);
// impl<'db> LookbackStateGetter<'db, MemoryDB> for MockStateLB<'db, MemoryDB> {
//     fn state_lookback(&self, _: ChainEpoch) -> Result<StateTree<'db, MemoryDB>, Box<dyn StdError>> {
//         Err("Lotus runner doesn't seem to initialize this?".into())
//     }
// }



pub fn execute_message<B: Blockstore>(
    bs: B,
    selector: &Option<Selector>,
    params: ExecuteMessageParams,
) -> Result<(ApplyRet, Cid), Box<dyn StdError>> {
    let circ_supply = MockCircSupply(params.circ_supply);

    let config = fvm::Config{
        initial_pages: 1024,
        max_pages: 4096,
        engine: Default::default()
    };
    let machine = fvm::machine::Machine::new(config, params.epoch, params.basefee, params, _, bs,  )

    // let mut vm = VM::<_, _, _, _, _>::new(
    //     params.pre_root,
    //     bs,
    //     params.epoch,
    //     &params.randomness,
    //     params.basefee,
    //     get_network_version_default,
    //     &circ_supply,
    //     &lb,
    // )?;

    // if let Some(s) = &selector {
    //     if s.chaos_actor
    //         .as_ref()
    //         .map(|s| s == "true")
    //         .unwrap_or_default()
    //     {
    //         vm.register_actor(*CHAOS_ACTOR_CODE_ID);
    //     }
    // }

    let ret = vm.apply_message(params.msg)?;

    let root = vm.flush()?;
    Ok((ret, root))
}
