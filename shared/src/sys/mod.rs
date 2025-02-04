//! This module contains types exchanged at the syscall layer between actors
//! (usually through the SDK) and the FVM.

pub mod out;

pub type BlockId = u32;
pub type Codec = u64;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TokenAmount {
    pub lo: u64,
    pub hi: u64,
}

impl From<TokenAmount> for crate::econ::TokenAmount {
    fn from(v: TokenAmount) -> Self {
        crate::econ::TokenAmount::from(v.hi) << 64 | crate::econ::TokenAmount::from(v.lo)
    }
}

impl TryFrom<crate::econ::TokenAmount> for TokenAmount {
    type Error = <crate::econ::TokenAmount as TryInto<u128>>::Error;
    fn try_from(v: crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        v.try_into().map(|v: u128| Self {
            hi: (v >> u64::BITS) as u64,
            lo: v as u64,
        })
    }
}

impl<'a> TryFrom<&'a crate::econ::TokenAmount> for TokenAmount {
    type Error = <&'a crate::econ::TokenAmount as TryInto<u128>>::Error;
    fn try_from(v: &'a crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        v.try_into().map(|v: u128| Self {
            hi: (v >> u64::BITS) as u64,
            lo: v as u64,
        })
    }
}
