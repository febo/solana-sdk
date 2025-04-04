//! Cross-program invocation helpers.

#[cfg(target_os = "solana")]
use solana_define_syscall::{
    define_syscall,
    definitions::{sol_invoke_signed_c, sol_set_return_data},
};
use {
    crate::{AccountMeta, InstructionView, MAX_INSTRUCTION_ACCOUNTS},
    core::{marker::PhantomData, mem::MaybeUninit, ops::Deref},
    solana_account_view::AccountView,
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};

#[cfg(target_os = "solana")]
define_syscall!(fn sol_get_return_data(data: *mut u8, length: u64, program_id: *mut Address) -> u64);

/// An `Instruction` as expected by `sol_invoke_signed_c`.
///
/// DO NOT EXPOSE THIS STRUCT:
///
/// To ensure pointers are valid upon use, the scope of this struct should
/// only be limited to the stack where `sol_invoke_signed_c`` happens and then
/// discarded immediately after.
#[repr(C)]
#[derive(Debug, PartialEq, Clone)]
struct CpiInstruction<'a> {
    /// Address of the program.
    program_id: *const Address,

    /// Accounts expected by the program instruction.
    accounts: *const AccountMeta<'a>,

    /// Number of accounts expected by the program instruction.
    accounts_len: u64,

    /// Data expected by the program instruction.
    data: *const u8,

    /// Length of the data expected by the program instruction.
    data_len: u64,
}

impl<'a> From<&InstructionView<'a, '_, '_, '_>> for CpiInstruction<'a> {
    fn from(instruction: &InstructionView<'a, '_, '_, '_>) -> Self {
        CpiInstruction {
            program_id: instruction.program_id,
            accounts: instruction.accounts.as_ptr(),
            accounts_len: instruction.accounts.len() as u64,
            data: instruction.data.as_ptr(),
            data_len: instruction.data.len() as u64,
        }
    }
}

/// An `Account` for CPI invocations.
///
/// This struct contains the same information as an [`AccountView`], but has
/// the memory layout as expected by `sol_invoke_signed_c` syscall.
#[repr(C)]
#[derive(Clone)]
pub struct CpiAccount<'a> {
    // Address of the account.
    key: *const Address,

    // Number of lamports owned by this account.
    lamports: *const u64,

    // Length of data in bytes.
    data_len: u64,

    // On-chain data within this account.
    data: *const u8,

    // Program that owns this account.
    owner: *const Address,

    // The epoch at which this account will next owe rent.
    rent_epoch: u64,

    // Transaction was signed by this account's key?
    is_signer: bool,

    // Is the account writable?
    is_writable: bool,

    // This account's data contains a loaded program (and is now read-only).
    executable: bool,

    /// The pointers to the `AccountView` data are only valid for as long as the
    /// `&'a AccountView` lives. Instead of holding a reference to the actual `AccountView`,
    /// which would increase the size of the type, we claim to hold a reference without
    /// actually holding one using a `PhantomData<&'a AccountView>`.
    _account_view: PhantomData<&'a AccountView>,
}

impl<'a> From<&'a AccountView> for CpiAccount<'a> {
    fn from(account: &'a AccountView) -> Self {
        CpiAccount {
            key: account.key(),
            lamports: &account.lamports(),
            data_len: account.data_len() as u64,
            // SAFETY: The caller ensures that the `AccountView` data is not mutably
            // borrowed, so that the data pointer is valid.
            data: unsafe { account.data_ptr() },
            // SAFETY: The caller ensures that the `AccountView` owner is valid.
            owner: unsafe { account.owner() },
            // The `rent_epoch` field is not present in the `AccountView` struct,
            // since the value occurs after the variable data of the account in
            // the runtime input data.
            rent_epoch: 0,
            is_signer: account.is_signer(),
            is_writable: account.is_writable(),
            executable: account.executable(),
            _account_view: PhantomData::<&'a AccountView>,
        }
    }
}

/// Invoke a cross-program instruction.
///
/// # Important
///
/// The accounts on the `account_infos` slice must be in the same order as the
/// `accounts` field of the `instruction`.
#[inline(always)]
pub fn invoke<const ACCOUNTS: usize>(
    instruction: &InstructionView,
    account_infos: &[&AccountView; ACCOUNTS],
) -> ProgramResult {
    invoke_signed(instruction, account_infos, &[])
}

/// Invoke a cross-program instruction from a slice of `AccountView`s.
///
/// # Important
///
/// The accounts on the `account_infos` slice must be in the same order as the
/// `accounts` field of the `instruction`.
#[inline(always)]
pub fn slice_invoke(
    instruction: &InstructionView,
    account_infos: &[&AccountView],
) -> ProgramResult {
    slice_invoke_signed(instruction, account_infos, &[])
}

/// Invoke a cross-program instruction with signatures.
///
/// # Important
///
/// The accounts on the `account_infos` slice must be in the same order as the
/// `accounts` field of the `instruction`.
pub fn invoke_signed<const ACCOUNTS: usize>(
    instruction: &InstructionView,
    account_infos: &[&AccountView; ACCOUNTS],
    signers_seeds: &[Signer],
) -> ProgramResult {
    if instruction.accounts.len() < ACCOUNTS {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    const UNINIT: MaybeUninit<CpiAccount> = MaybeUninit::<CpiAccount>::uninit();
    let mut accounts = [UNINIT; ACCOUNTS];

    for index in 0..ACCOUNTS {
        let account_info = account_infos[index];
        let account_meta = &instruction.accounts[index];

        if account_info.key() != account_meta.address {
            return Err(ProgramError::InvalidArgument);
        }

        if account_meta.is_writable {
            account_info.check_borrow_mut_data()?;
            account_info.check_borrow_mut_lamports()?;
        } else {
            account_info.check_borrow_data()?;
            account_info.check_borrow_lamports()?;
        }

        accounts[index].write(CpiAccount::from(account_infos[index]));
    }

    unsafe {
        invoke_signed_unchecked(
            instruction,
            core::slice::from_raw_parts(accounts.as_ptr() as _, ACCOUNTS),
            signers_seeds,
        );
    }

    Ok(())
}

/// Invoke a cross-program instruction with signatures from a slice of
/// `AccountView`s.
///
/// # Important
///
/// The accounts on the `account_infos` slice must be in the same order as the
/// `accounts` field of the `instruction`.
pub fn slice_invoke_signed(
    instruction: &InstructionView,
    account_infos: &[&AccountView],
    signers_seeds: &[Signer],
) -> ProgramResult {
    if instruction.accounts.len() < account_infos.len() {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    if account_infos.len() > MAX_INSTRUCTION_ACCOUNTS {
        return Err(ProgramError::InvalidArgument);
    }

    const UNINIT: MaybeUninit<CpiAccount> = MaybeUninit::<CpiAccount>::uninit();
    let mut accounts = [UNINIT; MAX_INSTRUCTION_ACCOUNTS];
    let mut len = 0;

    for (account_info, account_meta) in account_infos.iter().zip(instruction.accounts.iter()) {
        if account_info.key() != account_meta.address {
            return Err(ProgramError::InvalidArgument);
        }

        if account_meta.is_writable {
            account_info.check_borrow_mut_data()?;
            account_info.check_borrow_mut_lamports()?;
        } else {
            account_info.check_borrow_data()?;
            account_info.check_borrow_lamports()?;
        }
        // SAFETY: The number of accounts has been validated to be less than
        // `MAX_CPI_ACCOUNTS`.
        unsafe {
            accounts
                .get_unchecked_mut(len)
                .write(CpiAccount::from(*account_info));
        }

        len += 1;
    }
    // SAFETY: The accounts have been validated.
    unsafe {
        invoke_signed_unchecked(
            instruction,
            core::slice::from_raw_parts(accounts.as_ptr() as _, len),
            signers_seeds,
        );
    }

    Ok(())
}

/// Invoke a cross-program instruction but don't enforce Rust's aliasing rules.
///
/// This function does not check that [`Account`]s are properly borrowable.
/// Those checks consume CPU cycles that this function avoids.
///
/// # Safety
///
/// If any of the writable accounts passed to the callee contain data that is
/// borrowed within the calling program, and that data is written to by the
/// callee, then Rust's aliasing rules will be violated and cause undefined
/// behavior.
#[inline(always)]
pub unsafe fn invoke_unchecked(instruction: &InstructionView, accounts: &[CpiAccount]) {
    invoke_signed_unchecked(instruction, accounts, &[])
}

/// Invoke a cross-program instruction with signatures but don't enforce Rust's
/// aliasing rules.
///
/// This function does not check that [`Account`]s are properly borrowable.
/// Those checks consume CPU cycles that this function avoids.
///
/// # Safety
///
/// If any of the writable accounts passed to the callee contain data that is
/// borrowed within the calling program, and that data is written to by the
/// callee, then Rust's aliasing rules will be violated and cause undefined
/// behavior.
pub unsafe fn invoke_signed_unchecked(
    instruction: &InstructionView,
    accounts: &[CpiAccount],
    signers_seeds: &[Signer],
) {
    #[cfg(target_os = "solana")]
    {
        let instruction = CpiInstruction::from(instruction);
        unsafe {
            sol_invoke_signed_c(
                &instruction as *const _ as *const u8,
                accounts as *const _ as *const u8,
                accounts.len() as u64,
                signers_seeds as *const _ as *const u8,
                signers_seeds.len() as u64,
            )
        };
    }

    #[cfg(not(target_os = "solana"))]
    core::hint::black_box((instruction, accounts, signers_seeds));
}

/// Maximum size that can be set using [`set_return_data`].
pub const MAX_RETURN_DATA: usize = 1024;

/// Set the running program's return data.
///
/// Return data is a dedicated per-transaction buffer for data passed
/// from cross-program invoked programs back to their caller.
///
/// The maximum size of return data is [`MAX_RETURN_DATA`]. Return data is
/// retrieved by the caller with [`get_return_data`].
pub fn set_return_data(data: &[u8]) {
    #[cfg(target_os = "solana")]
    unsafe {
        sol_set_return_data(data.as_ptr(), data.len() as u64)
    };

    #[cfg(not(target_os = "solana"))]
    core::hint::black_box(data);
}

/// Get the return data from an invoked program.
///
/// For every transaction there is a single buffer with maximum length
/// [`MAX_RETURN_DATA`], paired with an [`Address`] representing the program ID of
/// the program that most recently set the return data. Thus the return data is
/// a global resource and care must be taken to ensure that it represents what
/// is expected: called programs are free to set or not set the return data; and
/// the return data may represent values set by programs multiple calls down the
/// call stack, depending on the circumstances of transaction execution.
///
/// Return data is set by the callee with [`set_return_data`].
///
/// Return data is cleared before every CPI invocation &mdash; a program that
/// has invoked no other programs can expect the return data to be `None`; if no
/// return data was set by the previous CPI invocation, then this function
/// returns `None`.
///
/// Return data is not cleared after returning from CPI invocations &mdash; a
/// program that has called another program may retrieve return data that was
/// not set by the called program, but instead set by a program further down the
/// call stack; or, if a program calls itself recursively, it is possible that
/// the return data was not set by the immediate call to that program, but by a
/// subsequent recursive call to that program. Likewise, an external RPC caller
/// may see return data that was not set by the program it is directly calling,
/// but by a program that program called.
///
/// For more about return data see the [documentation for the return data proposal][rdp].
///
/// [rdp]: https://docs.solanalabs.com/proposals/return-data
pub fn get_return_data() -> Option<ReturnData> {
    #[cfg(target_os = "solana")]
    {
        const UNINIT_BYTE: core::mem::MaybeUninit<u8> = core::mem::MaybeUninit::<u8>::uninit();
        let mut data = [UNINIT_BYTE; MAX_RETURN_DATA];
        let mut program_id: MaybeUninit<Address> = MaybeUninit::uninit();

        let size = unsafe {
            sol_get_return_data(
                data.as_mut_ptr() as *mut u8,
                data.len() as u64,
                program_id.as_mut_ptr() as *mut [u8; 32],
            )
        };

        if size == 0 {
            None
        } else {
            Some(ReturnData {
                program_id: unsafe { program_id.assume_init() },
                data,
                size: core::cmp::min(size as usize, MAX_RETURN_DATA),
            })
        }
    }

    #[cfg(not(target_os = "solana"))]
    core::hint::black_box(None)
}

/// Struct to hold the return data from an invoked program.
pub struct ReturnData {
    /// Program that most recently set the return data.
    program_id: Address,

    /// Return data set by the program.
    data: [core::mem::MaybeUninit<u8>; MAX_RETURN_DATA],

    /// Length of the return data.
    size: usize,
}

impl ReturnData {
    /// Returns the program that most recently set the return data.
    pub fn program_id(&self) -> &Address {
        &self.program_id
    }

    /// Return the data set by the program.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as _, self.size) }
    }
}

impl Deref for ReturnData {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Represents a signer seed.
///
/// This struct contains the same information as a `[u8]`, but
/// has the memory layout as expected by `sol_invoke_signed_c`
/// syscall.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Seed<'a> {
    /// Seed bytes.
    pub(crate) seed: *const u8,

    /// Length of the seed bytes.
    pub(crate) len: u64,

    /// The pointer to the seed bytes is only valid while the `&'a [u8]` lives. Instead
    /// of holding a reference to the actual `[u8]`, which would increase the size of the
    /// type, we claim to hold a reference without actually holding one using a
    /// `PhantomData<&'a [u8]>`.
    _bytes: PhantomData<&'a [u8]>,
}

impl<'a> From<&'a [u8]> for Seed<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self {
            seed: value.as_ptr(),
            len: value.len() as u64,
            _bytes: PhantomData::<&[u8]>,
        }
    }
}

impl<'a, const SIZE: usize> From<&'a [u8; SIZE]> for Seed<'a> {
    fn from(value: &'a [u8; SIZE]) -> Self {
        Self {
            seed: value.as_ptr(),
            len: value.len() as u64,
            _bytes: PhantomData::<&[u8]>,
        }
    }
}

impl Deref for Seed<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.seed, self.len as usize) }
    }
}

/// Represents a [program derived address][pda] (PDA) signer controlled by the
/// calling program.
///
/// [pda]: https://solana.com/docs/core/cpi#program-derived-addresses
#[repr(C)]
#[derive(Debug, Clone)]
pub struct Signer<'a, 'b> {
    /// Signer seeds.
    pub(crate) seeds: *const Seed<'a>,

    /// Number of seeds.
    pub(crate) len: u64,

    /// The pointer to the seeds is only valid while the `&'b [Seed<'a>]` lives. Instead
    /// of holding a reference to the actual `[Seed<'a>]`, which would increase the size
    /// of the type, we claim to hold a reference without actually holding one using a
    /// `PhantomData<&'b [Seed<'a>]>`.
    _seeds: PhantomData<&'b [Seed<'a>]>,
}

impl<'a, 'b> From<&'b [Seed<'a>]> for Signer<'a, 'b> {
    fn from(value: &'b [Seed<'a>]) -> Self {
        Self {
            seeds: value.as_ptr(),
            len: value.len() as u64,
            _seeds: PhantomData::<&'b [Seed<'a>]>,
        }
    }
}

impl<'a, 'b, const SIZE: usize> From<&'b [Seed<'a>; SIZE]> for Signer<'a, 'b> {
    fn from(value: &'b [Seed<'a>; SIZE]) -> Self {
        Self {
            seeds: value.as_ptr(),
            len: value.len() as u64,
            _seeds: PhantomData::<&'b [Seed<'a>]>,
        }
    }
}

/// Convenience macro for constructing a `[Seed; N]` array from a list of seeds.
///
/// # Example
///
/// Creating seeds array and signer for a PDA with a single seed and bump value:
/// ```
/// use pinocchio::{seeds, instruction::Signer};
/// use solana_address::Address;
///
/// let pda_bump = 0xffu8;
/// let pda_ref = &[pda_bump];  // prevent temporary value being freed
/// let example_key = Address::default();
/// let seeds = seeds!(b"seed", &example_key, pda_ref);
/// let signer = Signer::from(&seeds);
/// ```
#[macro_export]
macro_rules! seeds {
    ( $($seed:expr),* ) => {
        [$(
            $crate::instruction::Seed::from($seed),
        )*]
    };
}
