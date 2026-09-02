//! Configuration for network [rent].
//!
//! [rent]: https://docs.solanalabs.com/implemented-proposals/rent

#![allow(clippy::arithmetic_side_effects)]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(feature = "frozen-abi", feature(min_specialization))]
#[cfg(feature = "frozen-abi")]
extern crate std;

#[cfg(feature = "sysvar")]
pub mod sysvar;

#[cfg(feature = "wincode")]
use core::mem::MaybeUninit;
#[cfg(feature = "serde")]
use serde::ser::SerializeStruct;
#[cfg(feature = "frozen-abi")]
use solana_frozen_abi_macro::{AbiExample, StableAbi, StableAbiSample};
use solana_sdk_macro::CloneZeroed;
#[cfg(feature = "wincode")]
use wincode::{
    config::ConfigCore,
    io::{Reader, Writer},
    ReadResult, SchemaRead, SchemaWrite, WriteResult,
};

/// Configuration of network rent.
///
/// The `Rent` sysvar used to include `exemption_threshold` and `burn_percent` fields, but
/// these were deprecated and have been removed. The serialized size of the `Rent` sysvar
/// account is still `17` bytes, which is the size of the `Rent` sysvar account.
#[repr(C)]
#[cfg_attr(feature = "frozen-abi", derive(AbiExample, StableAbi, StableAbiSample))]
#[derive(PartialEq, CloneZeroed, Debug)]
pub struct Rent {
    /// Rental rate in lamports/byte.
    pub lamports_per_byte: u64,
}

/// Serialized size of the `Rent` sysvar account.
///
/// Note that this size represents the serialized size of the `Rent` sysvar,
/// which is 17 bytes. This includes:
///  - 8 bytes for `lamports_per_byte`
///  - 8 bytes for `exemption_threshold` (removed, but still counted in the size)
///  - 1 byte for `burn_percent` (removed, but still counted in the size)
pub const SIZE: usize = size_of::<u64>() // lamports_per_byte
    + size_of::<[u8; 8]>() // exemption_threshold
    + size_of::<u8>(); // burn_percent
const _: () = assert!(SIZE == 17);

/// Maximum permitted size of account data (10 MiB).
const MAX_PERMITTED_DATA_LENGTH: u64 = 10 * 1024 * 1024;

/// Maximum lamports per byte value.
const MAX_LAMPORTS_PER_BYTE: u64 = 1_759_197_129_867;

/// Default rental rate in lamports/byte.
///
/// This calculation is based on:
/// - 10^9 lamports per SOL
/// - $1 per SOL
/// - $0.01 per megabyte day
/// - $7.30 per megabyte
pub const DEFAULT_LAMPORTS_PER_BYTE: u64 = 6_960;

/// The `f64::to_le_bytes` representation of the SIMD-0194 exemption threshold.
///
/// This value is equivalent to `1.0f64`. It is only used to check whether
/// the exemption threshold is the deprecated value to avoid performing
/// floating-point operations on-chain.
#[cfg(any(feature = "serde", feature = "wincode"))]
const SIMD0194_EXEMPTION_THRESHOLD: [u8; 8] = [0, 0, 0, 0, 0, 0, 240, 63];

#[cfg(any(feature = "serde", feature = "wincode"))]
const DEFAULT_BURN_PERCENT: u8 = 50;

/// Account storage overhead for calculation of base rent.
///
/// This is the number of bytes required to store an account with no data. It is
/// added to an accounts data length when calculating [`Rent::minimum_balance`].
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

impl Default for Rent {
    fn default() -> Self {
        #[allow(deprecated)]
        Self {
            lamports_per_byte: DEFAULT_LAMPORTS_PER_BYTE,
        }
    }
}

impl Rent {
    /// Calculates the minimum balance for rent exemption.
    ///
    /// This method avoids floating-point operations when the `exemption_threshold`
    /// is the default value.
    ///
    /// # Arguments
    ///
    /// * `data_len` - The number of bytes in the account
    ///
    /// # Returns
    ///
    /// The minimum balance in lamports for rent exemption.
    ///
    /// # Panics
    ///
    /// Panics if `data_len` exceeds the maximum permitted data length or if the
    /// `lamports_per_byte` is too large based on the `exemption_threshold`.
    #[inline(always)]
    pub fn minimum_balance(&self, data_len: usize) -> u64 {
        self.try_minimum_balance(data_len)
            .expect("Maximum permitted data length exceeded")
    }

    /// Calculates the minimum balance for rent exemption without performing
    /// any validation.
    ///
    /// This method avoids floating-point operations when the `exemption_threshold`
    /// is the default value.
    ///
    /// # Important
    ///
    /// The caller must ensure that `data_len` is within the permitted limit
    /// and the `lamports_per_byte` is within the permitted limit based on
    /// the `exemption_threshold` to avoid overflow.
    ///
    /// # Arguments
    ///
    /// * `data_len` - The number of bytes in the account
    ///
    /// # Returns
    ///
    /// The minimum balance in lamports for rent exemption.
    #[inline(always)]
    pub fn minimum_balance_unchecked(&self, data_len: usize) -> u64 {
        (ACCOUNT_STORAGE_OVERHEAD + data_len as u64) * self.lamports_per_byte
    }

    /// Calculates the minimum balance for rent exemption.
    ///
    /// This method avoids floating-point operations when the `exemption_threshold`
    /// is the default value.
    ///
    /// # Arguments
    ///
    /// * `data_len` - The number of bytes in the account
    ///
    /// # Returns
    ///
    /// * `Some(u64)` - The minimum balance in lamports for rent exemption, if all checks pass.
    /// * `None` - If `data_len` exceeds the maximum permitted data length, or if the
    ///   `lamports_per_byte` is too large based on the `exemption_threshold`, which
    ///   would cause an overflow.
    #[inline(always)]
    pub fn try_minimum_balance(&self, data_len: usize) -> Option<u64> {
        if data_len as u64 > MAX_PERMITTED_DATA_LENGTH {
            return None;
        }

        // Validate `lamports_per_byte` based on `exemption_threshold`
        // to prevent overflow.

        if self.lamports_per_byte > MAX_LAMPORTS_PER_BYTE {
            return None;
        }

        Some(self.minimum_balance_unchecked(data_len))
    }

    /// Whether a given balance and data length would be exempt.
    pub fn is_exempt(&self, balance: u64, data_len: usize) -> bool {
        balance >= self.minimum_balance(data_len)
    }

    /// Creates a `Rent` that charges no lamports.
    ///
    /// This is used for testing.
    pub fn free() -> Self {
        Self {
            lamports_per_byte: 0,
        }
    }

    /// Creates a `Rent` with lamports per byte
    pub fn with_lamports_per_byte(lamports_per_byte: u64) -> Self {
        Self { lamports_per_byte }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Rent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde_derive::Deserialize)]
        struct RentWithDeprecatedFields {
            lamports_per_byte: u64,
            _exemption_threshold: [u8; 8],
            _burn_percent: u8,
        }

        let RentWithDeprecatedFields {
            lamports_per_byte,
            _exemption_threshold,
            _burn_percent,
        } = serde::Deserialize::deserialize(deserializer)?;

        Ok(Self { lamports_per_byte })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Rent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Rent", 3)?;
        state.serialize_field("lamports_per_byte", &self.lamports_per_byte)?;
        state.serialize_field("exemption_threshold", &SIMD0194_EXEMPTION_THRESHOLD)?;
        state.serialize_field("burn_percent", &DEFAULT_BURN_PERCENT)?;
        state.end()
    }
}

#[cfg(feature = "wincode")]
unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Rent {
    type Dst = Rent;

    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
        // SAFETY: `SIZE` is the serialized size of the `Rent` sysvar account,
        // which is 17 bytes.
        let mut reader = unsafe { reader.as_trusted_for(SIZE)? };
        let lamports_per_byte = <u64 as SchemaRead<C>>::get(reader.by_ref())?;
        // Consume the deprecated fields to maintain compatibility with the serialized
        // size of the `Rent` sysvar account.
        const DEPRECATED_FIELDS_SIZE: usize = SIZE - size_of::<u64>();
        let _ = reader.take_array::<DEPRECATED_FIELDS_SIZE>()?;

        dst.write(Rent { lamports_per_byte });

        Ok(())
    }
}

#[cfg(feature = "wincode")]
unsafe impl<C: ConfigCore> SchemaWrite<C> for Rent {
    type Src = Self;

    #[inline(always)]
    fn size_of(_src: &Self::Src) -> WriteResult<usize> {
        // The serialized `Rent` sysvar account is 17 bytes.
        Ok(SIZE)
    }

    fn write(mut writer: impl Writer, src: &Self::Src) -> WriteResult<()> {
        // SAFETY: `SIZE` is the serialized size of the `Rent` sysvar account,
        // which is 17 bytes.
        let mut writer = unsafe { writer.as_trusted_for(SIZE) }?;
        writer.write(&src.lamports_per_byte.to_le_bytes())?;
        writer.write(&SIMD0194_EXEMPTION_THRESHOLD)?;
        writer.write(&DEFAULT_BURN_PERCENT.to_le_bytes())?;
        writer.finish()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{sysvar, *},
        core::assert_eq,
        proptest::proptest,
        solana_account::{AccountSharedData, WritableAccount},
        wincode::{io::WriteError::WriteSizeLimit, WriteError},
    };

    #[test]
    fn test_size_of() {
        assert_eq!(
            wincode::serialized_size(&Rent::default()).unwrap() as usize,
            SIZE,
        );
    }

    #[test]
    fn test_clone() {
        #[allow(deprecated)]
        let rent = Rent {
            lamports_per_byte: 1,
        };
        #[allow(clippy::clone_on_copy)]
        let cloned_rent = rent.clone();
        assert_eq!(cloned_rent, rent);
    }

    proptest! {
        #[test]
        fn test_minimum_balance(bytes in 0usize..=MAX_PERMITTED_DATA_LENGTH as usize) {
            let default_rent = Rent::default();
            #[allow(deprecated)]
            let previous_rent = Rent {
                lamports_per_byte: DEFAULT_LAMPORTS_PER_BYTE / 2,
            };
            let default_calc = default_rent.minimum_balance(bytes);
            assert_eq!(default_calc, previous_rent.minimum_balance(bytes));
        }
    }

    #[test]
    fn test_deserialize() {
        let data = [1u8; 17];
        let rent: Rent = wincode::deserialize(&data).unwrap();
        assert_eq!(
            rent,
            Rent::with_lamports_per_byte(u64::from_le_bytes([1; 8]))
        );
    }

    #[test]
    fn test_serialize() {
        let rent = Rent::with_lamports_per_byte(1);
        let serialized_len =
            wincode::serialized_size(&rent).expect("failed to get serialized sysvar size") as usize;

        let mut account = AccountSharedData::new(
            rent.minimum_balance(serialized_len),
            serialized_len,
            &sysvar::id(),
        );
        wincode::serialize_into(account.data_as_mut_slice(), &rent).unwrap();

        assert_eq!(serialized_len, SIZE);
        assert_eq!(account.data_as_mut_slice().len(), serialized_len);
        assert_eq!(
            account.data_as_mut_slice()[..8],
            rent.lamports_per_byte.to_le_bytes()
        );
    }

    #[test]
    fn test_fail_serialize() {
        let rent = Rent::with_lamports_per_byte(1);
        // Wrongly using `size_of::<Rent>()` instead of `wincode::serialized_size(&rent)`
        // to get the serialized length
        let serialized_len = size_of::<Rent>();

        let mut account = AccountSharedData::new(
            rent.minimum_balance(serialized_len),
            serialized_len,
            &sysvar::id(),
        );
        let result = wincode::serialize_into(account.data_as_mut_slice(), &rent);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WriteError::Io(WriteSizeLimit(SIZE))
        ));
    }
}
