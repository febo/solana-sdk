use crate::{Address, MAX_SEEDS, PDA_MARKER};
#[cfg(target_os = "solana")]
use core::mem::MaybeUninit;

/// Derive a [program address][pda] from the given seeds, optional bump and
/// program id.
///
/// [pda]: https://solana.com/docs/core/pda
///
/// In general, the derivation uses an optional bump (byte) value to ensure a
/// valid PDA (off-curve) is generated. Even when a program stores a bump to
/// derive a program address, it is necessary to use the
/// [`pinocchio::pubkey::create_program_address`] to validate the derivation. In
/// most cases, the program has the correct seeds for the derivation, so it would
/// be sufficient to just perform the derivation and compare it against the
/// expected resulting address.
///
/// This function avoids the cost of the `create_program_address` syscall
/// (`1500` compute units) by directly computing the derived address
/// calculating the hash of the seeds, bump and program id using the
/// `sol_sha256` syscall.
///
/// # Important
///
/// This function differs from [`Address::create_program_address`] in that
/// it does not perform a validation to ensure that the derived address is a valid
/// (off-curve) program derived address. It is intended for use in cases where the
/// seeds, bump, and program id are known to be valid, and the caller wants to derive
/// the address without incurring the cost of the `create_program_address` syscall.
pub fn derive_address<const N: usize>(
    seeds: &[&[u8]; N],
    bump: Option<u8>,
    program_id: &Address,
) -> Address {
    #[cfg(target_os = "solana")]
    {
        const {
            assert!(N < MAX_SEEDS, "number of seeds must be less than MAX_SEEDS");
        }

        const UNINIT: MaybeUninit<&[u8]> = MaybeUninit::<&[u8]>::uninit();
        let mut data = [UNINIT; MAX_SEEDS + 2];
        let mut i = 0;

        while i < N {
            // SAFETY: `data` is guaranteed to have enough space for `N` seeds,
            // so `i` will always be within bounds.
            unsafe {
                data.get_unchecked_mut(i).write(seeds.get_unchecked(i));
            }
            i += 1;
        }

        // TODO: replace this with `as_slice` when the MSRV is upgraded
        // to `1.84.0+`.
        let bump_seed = [bump.unwrap_or_default()];

        // SAFETY: `data` is guaranteed to have enough space for `MAX_SEEDS + 2`
        // elements, and `MAX_SEEDS` is as large as `N`.
        unsafe {
            if bump.is_some() {
                data.get_unchecked_mut(i).write(&bump_seed);
                i += 1;
            }
            data.get_unchecked_mut(i).write(program_id.as_ref());
            data.get_unchecked_mut(i + 1).write(PDA_MARKER.as_ref());
        }

        let mut pda = MaybeUninit::<Address>::uninit();

        // SAFETY: `data` has `i + 2` elements initialized.
        unsafe {
            sol_sha256(
                data.as_ptr() as *const u8,
                (i + 2) as u64,
                pda.as_mut_ptr() as *mut u8,
            );
        }

        // SAFETY: `pda` has been initialized by the syscall.
        Address::new_from_array(unsafe { pda.assume_init() })
    }

    #[cfg(not(target_os = "solana"))]
    derive_address_const(seeds, bump, program_id)
}

/// Derive a [program address][pda] from the given seeds, optional bump and
/// program id.
///
/// [pda]: https://solana.com/docs/core/pda
///
/// In general, the derivation uses an optional bump (byte) value to ensure a
/// valid PDA (off-curve) is generated.
///
/// This function is intended for use in `const` contexts - i.e., the seeds and
/// bump are known at compile time and the program id is also a constant. It avoids
/// the cost of the `create_program_address` syscall (`1500` compute units) by
/// directly computing the derived address using the SHA-256 hash of the seeds,
/// bump and program id.
///
/// # Important
///
/// This function differs from [`Address::create_program_address`] in that
/// it does not perform a validation to ensure that the derived address is a valid
/// (off-curve) program derived address. It is intended for use in cases where the
/// seeds, bump, and program id are known to be valid, and the caller wants to derive
/// the address without incurring the cost of the `create_program_address` syscall.
///
/// This function is a compile-time constant version of [`derive_address`].
pub const fn derive_address_const<const N: usize>(
    seeds: &[&[u8]; N],
    bump: Option<u8>,
    program_id: &Address,
) -> Address {
    const {
        assert!(N < MAX_SEEDS, "number of seeds must be less than MAX_SEEDS");
    }

    let mut hasher = sha2_const_stable::Sha256::new();
    let mut i = 0;

    while i < seeds.len() {
        hasher = hasher.update(seeds[i]);
        i += 1;
    }

    // TODO: replace this with `is_some` when the MSRV is upgraded
    // to `1.84.0+`.
    Address::new_from_array(if let Some(bump) = bump {
        hasher
            .update(&[bump])
            .update(program_id.as_array())
            .update(PDA_MARKER)
            .finalize()
    } else {
        hasher
            .update(program_id.as_array())
            .update(PDA_MARKER)
            .finalize()
    })
}
