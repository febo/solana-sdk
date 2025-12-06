//! Logging wrapper for Solana syscalls.

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
use core::hint::black_box;
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::{sol_log_, sol_log_64_, sol_log_compute_units_};

/// Print a string to the log.
#[inline(always)]
pub fn sol_log(message: &str) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        sol_log_(message.as_ptr(), message.len() as u64);
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    black_box(message);
}

/// Print 64-bit values represented as hexadecimal to the log.
#[inline(always)]
pub fn sol_log_64(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        sol_log_64_(arg1, arg2, arg3, arg4, arg5);
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    black_box((arg1, arg2, arg3, arg4, arg5));
}

/// Print some slices as `base64`.
#[inline(always)]
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        solana_define_syscall::definitions::sol_log_data(
            data as *const _ as *const u8,
            data.len() as u64,
        )
    };

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    black_box(data);
}

/// Print the hexadecimal representation of a slice.
#[inline(always)]
pub fn sol_log_slice(slice: &[u8]) {
    for (i, s) in slice.iter().enumerate() {
        sol_log_64(0, 0, 0, i as u64, *s as u64);
    }
}

/// Print the remaining compute units available to the program.
#[inline]
pub fn sol_log_compute_units() {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    unsafe {
        sol_log_compute_units_();
    }
}
