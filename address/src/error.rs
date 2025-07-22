#[cfg(feature = "serde")]
use serde_derive::Serialize;
use {
    core::{convert::Infallible, fmt},
    num_traits::{FromPrimitive, ToPrimitive},
    solana_program_error::ProgramError,
};

// Use strum when testing to ensure our FromPrimitive
// impl is exhaustive
#[cfg_attr(test, derive(strum_macros::FromRepr, strum_macros::EnumIter))]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubkeyError {
    /// Length of the seed is too long for address generation
    MaxSeedLengthExceeded,
    InvalidSeeds,
    IllegalOwner,
}

impl ToPrimitive for PubkeyError {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        Some(match *self {
            PubkeyError::MaxSeedLengthExceeded => PubkeyError::MaxSeedLengthExceeded as i64,
            PubkeyError::InvalidSeeds => PubkeyError::InvalidSeeds as i64,
            PubkeyError::IllegalOwner => PubkeyError::IllegalOwner as i64,
        })
    }
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        self.to_i64().map(|x| x as u64)
    }
}

impl FromPrimitive for PubkeyError {
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {
        if n == PubkeyError::MaxSeedLengthExceeded as i64 {
            Some(PubkeyError::MaxSeedLengthExceeded)
        } else if n == PubkeyError::InvalidSeeds as i64 {
            Some(PubkeyError::InvalidSeeds)
        } else if n == PubkeyError::IllegalOwner as i64 {
            Some(PubkeyError::IllegalOwner)
        } else {
            None
        }
    }
    #[inline]
    fn from_u64(n: u64) -> Option<Self> {
        Self::from_i64(n as i64)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PubkeyError {}

impl fmt::Display for PubkeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PubkeyError::MaxSeedLengthExceeded => {
                f.write_str("Length of the seed is too long for address generation")
            }
            PubkeyError::InvalidSeeds => {
                f.write_str("Provided seeds do not result in a valid address")
            }
            PubkeyError::IllegalOwner => f.write_str("Provided owner is not allowed"),
        }
    }
}

impl From<u64> for PubkeyError {
    fn from(error: u64) -> Self {
        match error {
            0 => PubkeyError::MaxSeedLengthExceeded,
            1 => PubkeyError::InvalidSeeds,
            2 => PubkeyError::IllegalOwner,
            _ => panic!("Unsupported PubkeyError"),
        }
    }
}

impl From<PubkeyError> for ProgramError {
    fn from(error: PubkeyError) -> Self {
        match error {
            PubkeyError::MaxSeedLengthExceeded => Self::MaxSeedLengthExceeded,
            PubkeyError::InvalidSeeds => Self::InvalidSeeds,
            PubkeyError::IllegalOwner => Self::IllegalOwner,
        }
    }
}

// Use strum when testing to ensure our FromPrimitive
// impl is exhaustive
#[cfg_attr(test, derive(strum_macros::FromRepr, strum_macros::EnumIter))]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsePubkeyError {
    WrongSize,
    Invalid,
}

impl ToPrimitive for ParsePubkeyError {
    #[inline]
    fn to_i64(&self) -> Option<i64> {
        Some(match *self {
            ParsePubkeyError::WrongSize => ParsePubkeyError::WrongSize as i64,
            ParsePubkeyError::Invalid => ParsePubkeyError::Invalid as i64,
        })
    }
    #[inline]
    fn to_u64(&self) -> Option<u64> {
        self.to_i64().map(|x| x as u64)
    }
}

impl FromPrimitive for ParsePubkeyError {
    #[inline]
    fn from_i64(n: i64) -> Option<Self> {
        if n == ParsePubkeyError::WrongSize as i64 {
            Some(ParsePubkeyError::WrongSize)
        } else if n == ParsePubkeyError::Invalid as i64 {
            Some(ParsePubkeyError::Invalid)
        } else {
            None
        }
    }
    #[inline]
    fn from_u64(n: u64) -> Option<Self> {
        Self::from_i64(n as i64)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParsePubkeyError {}

impl fmt::Display for ParsePubkeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParsePubkeyError::WrongSize => f.write_str("String is the wrong size"),
            ParsePubkeyError::Invalid => f.write_str("Invalid Base58 string"),
        }
    }
}

impl From<Infallible> for ParsePubkeyError {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible uninhabited");
    }
}
