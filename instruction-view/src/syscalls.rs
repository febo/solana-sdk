//! Syscalls to query information about instructions.

#[cfg(target_os = "solana")]
use solana_define_syscall::{define_syscall, definitions::sol_get_stack_height};
use {
    crate::MAX_INSTRUCTION_ACCOUNTS,
    core::{mem::MaybeUninit, slice::from_raw_parts},
    solana_address::Address,
};

#[cfg(target_os = "solana")]
// Redefinition of the syscall to use different parameter types.
define_syscall!(fn sol_get_processed_sibling_instruction(index: u64, meta: *mut u8, program_id: *mut Address, data: *mut u8, accounts: *mut ProcessedAccount) -> u64);

/// Returns a sibling instruction from the processed sibling instruction list.
///
/// The processed sibling instruction list is a reverse-ordered list of
/// successfully processed sibling instructions. For example, given the call flow:
///
/// A
/// B -> C -> D
/// B -> E
/// B -> F
///
/// Then B's processed sibling instruction list is: `[A]`
/// Then F's processed sibling instruction list is: `[E, C]`
pub fn get_processed_sibling_instruction(index: usize) -> Option<ProcessedSiblingInstruction> {
    #[cfg(target_os = "solana")]
    {
        unsafe {
            let mut sibling = ProcessedSiblingInstruction::new_uninit();

            if 1 == sol_get_processed_sibling_instruction(
                index as u64,
                sibling.meta.as_mut_ptr() as *mut _,
                sibling.program_id.as_mut_ptr() as *mut _,
                sibling.data.as_mut_ptr() as *mut _,
                sibling.accounts.as_mut_ptr() as *mut _,
            ) {
                Some(sibling)
            } else {
                None
            }
        }
    }

    #[cfg(not(target_os = "solana"))]
    {
        core::hint::black_box(index);
        None
    }
}

/// The maximum size of a transaction, which serves as the maximum size of
/// instruction data.
const MAX_INSTRUCTION_DATA: usize = 1232;

/// Representation of a uninitialized byte.
#[cfg(target_os = "solana")]
const UNINIT_BYTE: MaybeUninit<u8> = MaybeUninit::uninit();

/// Representation of an uninitialized `ProcessedAccount`.
#[cfg(target_os = "solana")]
const UNINIT_PROCESSED_ACCOUNT: MaybeUninit<ProcessedAccount> = MaybeUninit::uninit();

/// Representation of a sibling instruction.
#[repr(C)]
pub struct ProcessedSiblingInstruction {
    /// Meta information about the sibling instruction:
    ///   1. `data_len`: length of the instruction data.
    ///   2. `accounts_len`: number of AccountMeta structures.
    meta: [u64; 2],

    /// Instruction data of the sibling instruction.
    ///
    /// The value of this field is initialized by the syscall.
    data: [MaybeUninit<u8>; MAX_INSTRUCTION_DATA],

    /// Accounts of the sibling instruction.
    ///
    /// The value of this field is initialized by the syscall.
    accounts: [MaybeUninit<ProcessedAccount>; MAX_INSTRUCTION_ACCOUNTS],

    /// Program that processed the instruction.
    ///
    /// The value of this field is initialized by the syscall.
    program_id: MaybeUninit<Address>,
}

impl ProcessedSiblingInstruction {
    #[cfg(target_os = "solana")]
    unsafe fn new_uninit() -> Self {
        Self {
            meta: [MAX_INSTRUCTION_DATA as u64, MAX_INSTRUCTION_ACCOUNTS as u64],
            data: [UNINIT_BYTE; MAX_INSTRUCTION_DATA],
            accounts: [UNINIT_PROCESSED_ACCOUNT; MAX_INSTRUCTION_ACCOUNTS],
            program_id: MaybeUninit::uninit(),
        }
    }

    /// Returns the list of processed accounts.
    pub fn accounts(&self) -> &[ProcessedAccount] {
        // SAFETY: Account length is guaranteed to be less than or equal to
        // `MAX_INSTRUCTION_ACCOUNTS`.
        unsafe {
            let len = *self.meta.get_unchecked(1) as usize;
            from_raw_parts(self.accounts.get_unchecked(..len).as_ptr() as _, len)
        }
    }

    /// Returns the instruction data.
    pub fn instruction_data(&self) -> &[u8] {
        // SAFETY: Instruction data length is guaranteed to be less than
        // `MAX_INSTRUCTION_DATA`.
        unsafe {
            let len = *self.meta.get_unchecked(0) as usize;
            from_raw_parts(self.data.get_unchecked(..len).as_ptr() as _, len)
        }
    }

    /// Returns the address of the program that executed the instruction.
    pub fn program_id(&self) -> &Address {
        // SAFETY: The syscall initialized the program address.
        unsafe { self.program_id.assume_init_ref() }
    }
}

/// Representation of a processed account.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct ProcessedAccount {
    /// The address of the account.
    pub key: Address,

    /// Indicates whether the account is signer or not.
    pub is_signer: bool,

    /// Indicates whether the account is writable or not.
    pub is_writable: bool,
}

// Stack height when processing transaction-level instructions.
pub const TRANSACTION_LEVEL_STACK_HEIGHT: usize = 1;

/// Get the current stack height.
///
/// Transaction-level instructions are height [`TRANSACTION_LEVEL_STACK_HEIGHT`]`,
/// fist invoked inner instruction is height `TRANSACTION_LEVEL_STACK_HEIGHT + 1`,
/// and so forth.
pub fn get_stack_height() -> usize {
    #[cfg(target_os = "solana")]
    unsafe {
        sol_get_stack_height() as usize
    }

    #[cfg(not(target_os = "solana"))]
    panic!("get_stack_height is only available on target `solana`")
}
