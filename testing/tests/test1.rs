mod common;

use crate::common::DummyExterns;
use blockstore::{Blockstore, MemoryBlockstore};
use cid::Cid;
use fvm::machine::Machine;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use std::borrow::Borrow;
use std::error::Error;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Mutex;
use fvm::init_actor::State;
use fvm::state_tree::{ActorState, StateTree};
use fvm_shared::HAMT_BIT_WIDTH;
use fvm_shared::state::StateTreeVersion;

#[test]
pub fn test() -> Result<(), Box<dyn Error>> {
    let config = fvm::Config {
        initial_pages: 16,
        max_pages: 1024,
        engine: Default::default(),
    };
    let epoch = 1_000_000 as ChainEpoch;
    let base_fee = TokenAmount::from(1_000);
    let nv = NetworkVersion::V13; // latest
    let bs = MemoryBlockstore::default();
    let ext = DummyExterns;
    let cid = Cid::default();
    bs.put(&cid, vec![].as_slice())?;

    let mut hamt = ipld_hamt::Hamt::new_with_bit_width(&bs, HAMT_BIT_WIDTH);
    let empty = hamt.flush()?;

    let mut tree = StateTree::new(&store, StateTreeVersion::V3)?;

    let init_state = State::new(&bs, String::from("test"));
    let state_cid = tree.store().put() // ipld blockstore

    state_tree.set_actor(&fvm::init_actor::INIT_ACTOR_ADDR, ActorState{
        code: *fvm::init_actor::INIT_ACTOR_CODE_ID,
        state: ipld_hamt::,
        sequence: 0,
        balance: Default::default(),
    });


    // Empty hamt Cid used for testing
    let e_cid = Hamt::<_, String>::new_with_bit_width(&store, 5)
        .flush()
        .unwrap();

    let init_state = init::State::new(e_cid, "test".to_owned());
    let state_cid = tree
        .store()
        .put(&init_state, Blake2b256)
        .map_err(|e| e.to_string())
        .unwrap();

    let act_s = ActorState::new(*INIT_ACTOR_CODE_ID, state_cid, Default::default(), 1);

    tree.snapshot().unwrap();
    tree.set_actor(&INIT_ACTOR_ADDR, act_s).unwrap();

    // Test mutate function
    tree.mutate_actor(&INIT_ACTOR_ADDR, |mut actor| {
        actor.sequence = 2;
        Ok(())
    })



    let machine: Machine<MemoryBlockstore, DummyExterns> =
        fvm::machine::Machine::new(config)?;

    Ok(())
}
