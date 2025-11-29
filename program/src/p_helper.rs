use pinocchio::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{pubkey_eq, Pubkey},
};
#[inline(always)]
pub fn p_load_mut_unchecked<T>(acc_info: &AccountInfo) -> Result<&mut T, ProgramError> {
    let data = unsafe { acc_info.borrow_mut_data_unchecked() };
    Ok(unsafe { &mut *(data[..].as_mut_ptr() as *mut T) })
}
#[inline(always)]
pub fn p_transfer_from_user(
    authority: &AccountInfo,
    token_owner_account: &AccountInfo,
    destination_token_account: &AccountInfo,
    token_program: &AccountInfo,
    amount: u64,
) -> ProgramResult {
    pinocchio_token_2022::instructions::Transfer {
        from: token_owner_account,
        to: destination_token_account,
        authority,
        amount,
        token_program: token_program.key(),
    }
    .invoke()
}
#[inline(always)]
pub fn p_transfer_from_pool(
    pool_authority: &AccountInfo,
    token_vault: &AccountInfo,
    token_owner_account: &AccountInfo,
    token_program: &AccountInfo,
    amm_seed: &[u8],
    nonce: u8,
    amount: u64,
) -> ProgramResult {
    let nonce = &[nonce];
    pinocchio_token_2022::instructions::Transfer {
        from: token_vault,
        to: token_owner_account,
        authority: pool_authority,
        amount,
        token_program: token_program.key(),
    }
    .invoke_signed(&[Signer::from(&pinocchio::seeds!(amm_seed, nonce))])
}
#[inline(always)]
pub fn p_cmp_mint(a: &AccountInfo, b: &AccountInfo) -> bool {
    unsafe {
        pubkey_eq(
            &*(a.borrow_data_unchecked().as_ptr() as *const Pubkey),
            &*(b.borrow_data_unchecked().as_ptr() as *const Pubkey),
        )
    }
}
#[inline(always)]
pub fn p_acessor_balance(token_acc: &AccountInfo) -> u64 {
    u64::from_le_bytes(
        unsafe { &token_acc.borrow_data_unchecked()[64..72] }
            .try_into()
            .unwrap(),
    )
}
