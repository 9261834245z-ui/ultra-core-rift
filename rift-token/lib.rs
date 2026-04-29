use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use ultra_core_rift::program::UltraCoreRift;
use ultra_core_rift::{CoreState, RiftError};

declare_id!("RiftToken111111111111111111111111111111111");

pub const FOUNDER_SHARE_BPS: u16 = 314;
pub const MAX_FEE_BPS: u16 = 10;

#[error_code]
pub enum TokenError {
    #[msg("Fee exceeds maximum protocol limits.")]
    FeeTooHigh,
    #[msg("Invalid admin vault address.")]
    InvalidAdminVault,
}

#[program]
pub mod rift_token {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        decimals: u8,
        fee_bps: u16,
        initial_supply: u64,
    ) -> Result<()> {
        require!(fee_bps <= MAX_FEE_BPS, TokenError::FeeTooHigh);

        let state = &mut ctx.accounts.rift_token_state;
        state.authority = ctx.accounts.gate.key();
        state.core_state = ctx.accounts.core_state.key();
        state.admin_vault = ctx.accounts.admin_vault.key();
        state.decimals = decimals;
        state.fee_bps = fee_bps;
        state.total_shares = 0;
        state.rift_multiplier = 1_000_000_000_000_000u128;
        state.bump = ctx.bumps.rift_token_state;

        let founder_share = (initial_supply as u128)
            .checked_mul(FOUNDER_SHARE_BPS as u128)
            .ok_or(RiftError::MathOverflow)?
            .checked_div(10_000)
            .unwrap_or(0) as u64;

        if founder_share > 0 {
            let auth_bump = ctx.bumps.rift_authority;
            let signer_seeds: &[&[&[u8]]] = &[&[b"rift_mint_authority", &[auth_bump]]];

            let cpi_accounts = token::MintTo {
                mint: ctx.accounts.rift_mint.to_account_info(),
                to: ctx.accounts.admin_vault_token_account.to_account_info(),
                authority: ctx.accounts.rift_authority.to_account_info(),
            };

            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer_seeds,
            );

            token::mint_to(cpi_ctx, founder_share)?;

            state.total_shares = state.total_shares
                .checked_add(founder_share)
                .ok_or(RiftError::MathOverflow)?;
        }

        Ok(())
    }

    pub fn issue_rift(ctx: Context<IssueRift>, base_amount: u64) -> Result<()> {
        let core = &ctx.accounts.core_state;
        core.check_invariant()?;

        let state = &ctx.accounts.rift_token_state;

        let fee_amount = (base_amount as u128)
            .checked_mul(state.fee_bps as u128)
            .ok_or(RiftError::MathOverflow)?
            .checked_div(10_000)
            .unwrap_or(0) as u64;

        let amount_after_fee = base_amount
            .checked_sub(fee_amount)
            .ok_or(RiftError::MathOverflow)?;

        anchor_lang::solana_program::program::invoke(
            &anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.user.key(),
                &ctx.accounts.admin_vault.key(),
                fee_amount,
            ),
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.admin_vault.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Safe math: unsigned_abs to prevent panic on i128::MIN
        let field_pressure = core.global_field.unsigned_abs().max(1) as u128;

        let mint_multiplier = 1_000_000_000_000_000u128
            .checked_div(field_pressure)
            .unwrap_or(1_000_000_000_000u128);

        let shares_to_mint = (amount_after_fee as u128)
            .checked_mul(mint_multiplier)
            .ok_or(RiftError::MathOverflow)?
            .checked_div(1_000_000_000_000u128)
            .unwrap_or(0) as u64;

        let auth_bump = ctx.bumps.rift_authority;
        let signer_seeds: &[&[&[u8]]] = &[&[b"rift_mint_authority", &[auth_bump]]];

        let cpi_accounts = token::MintTo {
            mint: ctx.accounts.rift_mint.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.rift_authority.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );

        token::mint_to(cpi_ctx, shares_to_mint)?;

        state.total_shares = state.total_shares
            .checked_add(shares_to_mint)
            .ok_or(RiftError::MathOverflow)?;

        emit!(IssueRiftEvent {
            user: ctx.accounts.user.key(),
            base_amount,
            fee_amount,
            shares_minted: shares_to_mint,
            global_field: core.global_field,
            rift_multiplier: state.rift_multiplier,
        });

        Ok(())
    }

    pub fn rebase(ctx: Context<Rebase>) -> Result<()> {
        let core = &ctx.accounts.core_state;
        let state = &mut ctx.accounts.rift_token_state;

        core.check_invariant()?;

        let field_pressure = core.global_field.unsigned_abs().max(1) as u128;

        let new_multiplier = 1_000_000_000_000_000u128
            .checked_div(field_pressure)
            .unwrap_or(1_000_000_000_000u128);

        let old_multiplier = state.rift_multiplier;
        state.rift_multiplier = new_multiplier;

        emit!(RiftRebaseEvent {
            old_multiplier,
            new_multiplier,
            global_field: core.global_field,
        });

        Ok(())
    }
}

// ========================== STATE ==========================

#[account]
pub struct RiftTokenState {
    pub authority: Pubkey,
    pub core_state: Pubkey,
    pub admin_vault: Pubkey,
    pub decimals: u8,
    pub fee_bps: u16,
    pub total_shares: u64,
    pub rift_multiplier: u128,
    pub bump: u8,
}

// ========================== CONTEXTS ==========================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = gate, space = 8 + 32*3 + 1 + 2 + 8 + 16 + 1)]
    pub rift_token_state: Account<'info, RiftTokenState>,

    #[account(mut)]
    pub core_state: Account<'info, CoreState>,

    #[account(mut)]
    pub rift_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin_vault_token_account: Account<'info, TokenAccount>,

    /// CHECK: Admin vault that receives genesis share and protocol fees. Set by gate authority during initialization.
    pub admin_vault: UncheckedAccount<'info>,

    /// CHECK: PDA used as mint authority for RIFT tokens.
    #[account(seeds = [b"rift_mint_authority"], bump)]
    pub rift_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub gate: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct IssueRift<'info> {
    #[account(mut)]
    pub rift_token_state: Account<'info, RiftTokenState>,

    pub core_state: Account<'info, CoreState>,

    #[account(mut)]
    pub rift_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// CHECK: PDA mint authority. Signed via seeds in CPI.
    #[account(seeds = [b"rift_mint_authority"], bump)]
    pub rift_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: Receives protocol fees in SOL. Constrained to match stored admin_vault.
    #[account(mut, constraint = admin_vault.key() == rift_token_state.admin_vault @ TokenError::InvalidAdminVault)]
    pub admin_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Rebase<'info> {
    #[account(mut)]
    pub rift_token_state: Account<'info, RiftTokenState>,
    pub core_state: Account<'info, CoreState>,
    pub gate: Signer<'info>,
}

// ========================== EVENTS ==========================

#[event]
pub struct IssueRiftEvent {
    pub user: Pubkey,
    pub base_amount: u64,
    pub fee_amount: u64,
    pub shares_minted: u64,
    pub global_field: i128,
    pub rift_multiplier: u128,
}

#[event]
pub struct RiftRebaseEvent {
    pub old_multiplier: u128,
    pub new_multiplier: u128,
    pub global_field: i128,
}
