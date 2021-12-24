// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::*;
use crate::kernel::TestFallbackRand;
use crate::vector::{RandomnessKind, RandomnessMatch, RandomnessRule};
use fvm::externs::Rand;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use std::error::Error as StdError;

/// Takes recorded randomness and replays it when input parameters match.
/// When there's no match, it falls back to TestFallbackRand, which returns a
/// fixed output.
pub struct ReplayingRand<'a> {
    pub recorded: &'a [RandomnessMatch],
    pub fallback: TestFallbackRand,
}

/// Implements the Rand extern and returns static values as randomness outputs.
/// This is used by the
pub struct TestFallbackRand;

impl Rand for TestFallbackRand {
    fn get_chain_randomness(
        &self,
        _: DomainSeparationTag,
        _: ChainEpoch,
        _: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }

    fn get_chain_randomness_looking_forward(
        &self,
        _: DomainSeparationTag,
        _: ChainEpoch,
        _: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }

    fn get_beacon_randomness(
        &self,
        _: DomainSeparationTag,
        _: ChainEpoch,
        _: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }

    fn get_beacon_randomness_looking_forward(
        &self,
        _: DomainSeparationTag,
        _: ChainEpoch,
        _: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        Ok(*b"i_am_random_____i_am_random_____")
    }
}

impl<'a> ReplayingRand<'a> {
    pub fn new(recorded: &'a [RandomnessMatch]) -> Self {
        Self {
            recorded,
            fallback: TestFallbackRand,
        }
    }

    pub fn matches(&self, requested: RandomnessRule) -> Option<[u8; 32]> {
        for other in self.recorded {
            if other.on == requested {
                let mut randomness = [0u8; 32];
                randomness.copy_from_slice(&other.ret);
                return Some(randomness);
            }
        }
        None
    }
}

impl Rand for ReplayingRand<'_> {
    fn get_chain_randomness(
        &self,
        dst: DomainSeparationTag,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Chain,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_chain_randomness_v1(dst, epoch, entropy)
        }
    }
    // TODO: Check if this is going to be correct for when we integrate v5 Actors test vectors
    fn get_chain_randomness_looking_forward(
        &self,
        dst: DomainSeparationTag,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Chain,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_chain_randomness_v2(dst, epoch, entropy)
        }
    }
    fn get_beacon_randomness(
        &self,
        dst: DomainSeparationTag,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Beacon,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_beacon_randomness_v1(dst, epoch, entropy)
        }
    }
    // TODO: Check if this is going to be correct for when we integrate v5 Actors test vectors
    fn get_beacon_randomness_looking_forward(
        &self,
        dst: DomainSeparationTag,
        epoch: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        let rule = RandomnessRule {
            kind: RandomnessKind::Beacon,
            dst,
            epoch,
            entropy: entropy.to_vec(),
        };
        if let Some(bz) = self.matches(rule) {
            Ok(bz)
        } else {
            self.fallback.get_beacon_randomness_v2(dst, epoch, entropy)
        }
    }
}
