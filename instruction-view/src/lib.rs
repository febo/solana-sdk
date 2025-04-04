//! Lightweight types for directing the execution of Solana programs.
//!
//! This crate offers views and zero-copy types to interact with program
//! instructions and accounts. As a result, it reduces compute units
//! consumption. This is achieved by defining types that hold references
//! instead of owning the required data.

#![no_std]

use solana_address::Address;

#[cfg(feature = "cpi")]
pub mod cpi;
#[cfg(feature = "syscalls")]
pub mod syscalls;
#[cfg(feature = "sysvar")]
pub mod sysvar;

/// Maximum number of accounts that can be passed to an instruction.
#[cfg(any(feature = "syscalls", feature = "cpi"))]
pub const MAX_INSTRUCTION_ACCOUNTS: usize = 64;

/// Information about an instruction.
#[derive(Debug, Clone)]
pub struct InstructionView<'a, 'b, 'c, 'd>
where
    'a: 'b,
{
    /// Address of the program.
    pub program_id: &'c Address,

    /// Data expected by the program instruction.
    pub data: &'d [u8],

    /// Metadata describing accounts that should be passed to the program.
    pub accounts: &'b [AccountMeta<'a>],
}

/// Describes a single account read or written by a program during instruction
/// execution.
///
/// When constructing an [`InstructionView`], a list of all accounts that may be
/// read or written during the execution of that instruction must be supplied.
/// Any account that may be mutated by the program during execution, either its
/// data or metadata such as held lamports, must be writable.
///
/// Note that because the Solana runtime schedules parallel transaction
/// execution around which accounts are writable, care should be taken that only
/// accounts which actually may be mutated are specified as writable.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct AccountMeta<'a> {
    /// Address of the account.
    pub address: &'a Address,

    /// Indicates whether the account is writable or not.
    pub is_writable: bool,

    /// Indicates whether the account signed the instruction or not.
    pub is_signer: bool,
}

impl<'a> AccountMeta<'a> {
    /// Creates a new `AccountMeta`.
    #[inline(always)]
    pub fn new(address: &'a Address, is_writable: bool, is_signer: bool) -> Self {
        Self {
            address,
            is_writable,
            is_signer,
        }
    }

    /// Creates a new readonly `AccountMeta`.
    #[inline(always)]
    pub fn readonly(address: &'a Address) -> Self {
        Self::new(address, false, false)
    }

    /// Creates a new writable `AccountMeta`.
    #[inline(always)]
    pub fn writable(address: &'a Address) -> Self {
        Self::new(address, true, false)
    }

    /// Creates a new readonly and signer `AccountMeta`.
    #[inline(always)]
    pub fn readonly_signer(address: &'a Address) -> Self {
        Self::new(address, false, true)
    }

    /// Creates a new writable and signer `AccountMeta`.
    #[inline(always)]
    pub fn writable_signer(address: &'a Address) -> Self {
        Self::new(address, true, true)
    }
}
