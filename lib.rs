use anchor_lang::prelude::*;

declare_id!("RiftCore11111111111111111111111111111111111");

// ============================================================================
// CONSTANTS
// ============================================================================
pub const NEG_E: i128 = -2_718_281_828_459_045_235;
pub const MAX_PARTICIPANTS: u64 = 1_000_000_000_000;
pub const MAX_EDGE_COST: i128 = 1_000_000_000_000_000_000_000;
pub const MIN_ABS_DEBT: i128 = -1_000_000_000_000_000_000;
pub const MAX_SUPPLY: u128 = i128::MAX as u128;

#[program]
pub mod ultra_core_rift {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, gate: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.core_state;
        *state = CoreState {
            gate,
            paused: false,
            global_field: 0,
            total_base_sum: 0,
            total_supply: 0,
            total_minted: 0,
            total_burned: 0,
            p: 0,
            dust_accumulator: 0,
        };
        state.check_invariant()
    }

    pub fn set_edge(ctx: Context<SetEdge>, _from: Pubkey, _to: Pubkey, weight: i128) -> Result<()> {
        require!(weight >= -MAX_EDGE_COST && weight <= MAX_EDGE_COST, RiftError::EdgeLimitExceeded);
        ctx.accounts.edge_account.weight = weight;
        Ok(())
    }

    pub fn register(ctx: Context<Register>, user: Pubkey) -> Result<()> {
        let state = &mut ctx.accounts.core_state;
        require!(state.p < MAX_PARTICIPANTS, RiftError::MaxParticipantsReached);

        let user_account = &mut ctx.accounts.user_account;
        user_account.authority = user;
        user_account.base_balance = 0;

        state.total_base_sum = state.total_base_sum
            .checked_sub(state.global_field)
            .ok_or(RiftError::MathOverflow)?;

        state.p = state.p.checked_add(1).ok_or(RiftError::MathOverflow)?;

        emit!(RegisteredEvent { user });
        state.check_invariant()
    }

    pub fn unregister(ctx: Context<Unregister>) -> Result<()> {
        let state = &mut ctx.accounts.core_state;
        let base = ctx.accounts.user_account.base_balance;

        require!(base >= 0, RiftError::DebtOnExitNotAllowed);

        if base > 0 {
            let burn = base as u128;
            require!(state.total_supply >= burn, RiftError::SupplyUnderflow);

            state.total_supply = state.total_supply
                .checked_sub(burn)
                .ok_or(RiftError::MathOverflow)?;
            state.total_burned = state.total_burned
                .checked_add(burn)
                .ok_or(RiftError::MathOverflow)?;

            emit!(BurnEvent {
                user: ctx.accounts.user_account.authority,
                amount: burn,
            });
        }

        state.total_base_sum = state.total_base_sum
            .checked_sub(base)
            .ok_or(RiftError::MathOverflow)?
            .checked_add(state.global_field)
            .ok_or(RiftError::MathOverflow)?;

        state.p = state.p.checked_sub(1).ok_or(RiftError::MathOverflow)?;

        emit!(UnregisteredEvent { user: ctx.accounts.user_account.authority });
        state.check_invariant()
    }

    pub fn transfer(ctx: Context<Transfer>, amount: u128) -> Result<()> {
        ctx.accounts.transfer_ctx.perform_transfer(amount, 0)
    }

    pub fn transfer_with_edge(ctx: Context<TransferWithEdge>, amount: u128) -> Result<()> {
        let edge_cost = ctx.accounts.edge_account.weight;

        require_keys_eq!(
            ctx.accounts.transfer_ctx.to_user.authority,
            ctx.accounts.transfer_ctx.to_authority.key(),
            RiftError::UnauthorizedAuthority
        );

        ctx.accounts.transfer_ctx.perform_transfer(amount, edge_cost)
    }

    pub fn redistribute(ctx: Context<Redistribute>, amount: u128) -> Result<()> {
        let state = &mut ctx.accounts.core_state;
        require!(state.p > 0, RiftError::ZeroParticipants);

        let p_u128 = state.p as u128;
        let total = amount
            .checked_add(state.dust_accumulator)
            .ok_or(RiftError::MathOverflow)?;

        let q = total.checked_div(p_u128).ok_or(RiftError::MathOverflow)?;
        let r = total.checked_rem(p_u128).ok_or(RiftError::MathOverflow)?;

        state.global_field = state.global_field
            .checked_add(q as i128)
            .ok_or(RiftError::MathOverflow)?;

        let distributed = q.checked_mul(p_u128).ok_or(RiftError::MathOverflow)?;

        state.total_supply = state.total_supply
            .checked_add(distributed)
            .ok_or(RiftError::MathOverflow)?;
        state.total_minted = state.total_minted
            .checked_add(distributed)
            .ok_or(RiftError::MathOverflow)?;

        state.dust_accumulator = r;

        emit!(RedistributeEvent { amount, per_user: q, dust_retained: r });
        emit!(FieldUpdateEvent { new_global_field: state.global_field });

        state.check_invariant()
    }

    pub fn apply_neg_entropy(ctx: Context<ApplyNegEntropy>) -> Result<()> {
        let state = &mut ctx.accounts.core_state;

        let p_i128 = state.p as i128;
        let delta = p_i128
            .checked_mul(NEG_E)
            .ok_or(RiftError::MathOverflow)?;

        let max_p = i128::MAX
            .checked_div(-NEG_E)
            .ok_or(RiftError::MathOverflow)?;
        require!(p_i128 <= max_p, RiftError::PhysicalOverflowLimit);

        state.global_field = state.global_field
            .checked_add(NEG_E)
            .ok_or(RiftError::MathOverflow)?;

        state.total_base_sum = state.total_base_sum
            .checked_sub(delta)
            .ok_or(RiftError::MathOverflow)?;

        emit!(FieldUpdateEvent { new_global_field: state.global_field });
        state.check_invariant()
    }
}

// ============================================================================
// CORE STATE
// ============================================================================

#[account]
pub struct CoreState {
    pub gate: Pubkey,
    pub paused: bool,
    pub global_field: i128,
    pub total_base_sum: i128,
    pub total_supply: u128,
    pub total_minted: u128,
    pub total_burned: u128,
    pub p: u64,
    pub dust_accumulator: u128,
}

impl CoreState {
    pub const SPACE: usize = 8 + 32 + 1 + 16 * 6 + 8 + 16;

    /// Debt limit exactly matching the shadow model in fuzz tests
    pub fn debt_limit(&self) -> Result<i128> {
        let factor = (self.p as i128)
            .checked_mul(10)
            .ok_or(RiftError::MathOverflow)?;

        if factor == 0 {
            Ok(MIN_ABS_DEBT)
        } else {
            let limit = (self.total_supply as i128)
                .checked_div(factor)
                .ok_or(RiftError::MathOverflow)?;
            Ok(-limit)
        }
    }

    pub fn check_invariant(&self) -> Result<()> {
        require!(self.total_supply <= MAX_SUPPLY, RiftError::MathOverflow);

        let field_contrib = self.global_field
            .checked_mul(self.p as i128)
            .ok_or(RiftError::MathOverflow)?;

        let expected = self.total_base_sum
            .checked_add(field_contrib)
            .ok_or(RiftError::MathOverflow)?;

        let supply_signed = self.total_supply as i128;
        require!(supply_signed == expected, RiftError::InvariantViolation);

        require!(self.total_minted >= self.total_burned, RiftError::InvariantViolation);
        require!(
            self.total_supply == self.total_minted - self.total_burned,
            RiftError::InvariantViolation
        );

        if self.p > 0 {
            require!(
                self.dust_accumulator < self.p as u128,
                RiftError::InvariantViolation
            );
        }
        Ok(())
    }
}

// ============================================================================
// TRANSFER LOGIC
// ============================================================================

#[derive(Accounts)]
pub struct TransferCtx<'info> {
    #[account(mut)]
    pub core_state: Account<'info, CoreState>,
    #[account(mut, seeds = [b"user", from_authority.key().as_ref()], bump)]
    pub from_user: Account<'info, UserAccount>,
    #[account(mut, seeds = [b"user", to_authority.key().as_ref()], bump)]
    pub to_user: Account<'info, UserAccount>,
    pub from_authority: Signer<'info>,
    pub to_authority: UncheckedAccount<'info>,
}

impl<'info> TransferCtx<'info> {
    pub fn perform_transfer(&mut self, amount: u128, edge_cost: i128) -> Result<()> {
        let state = &mut self.core_state;
        require!(!state.paused, RiftError::ProtocolPaused);
        if amount == 0 {
            return Ok(());
        }

        let amt: i128 = amount.try_into().map_err(|_| RiftError::MathOverflow)?;

        let new_from = self.from_user.base_balance
            .checked_sub(amt)
            .ok_or(RiftError::MathOverflow)?
            .checked_sub(edge_cost)
            .ok_or(RiftError::MathOverflow)?;

        require!(new_from >= state.debt_limit()?, RiftError::DebtLimitExceeded);

        self.from_user.base_balance = new_from;
        self.to_user.base_balance = self.to_user.base_balance
            .checked_add(amt)
            .ok_or(RiftError::MathOverflow)?;

        if edge_cost != 0 {
            state.total_base_sum = state.total_base_sum
                .checked_sub(edge_cost)
                .ok_or(RiftError::MathOverflow)?;

            match edge_cost.cmp(&0) {
                std::cmp::Ordering::Greater => {
                    let burn = edge_cost as u128;
                    require!(state.total_supply >= burn, RiftError::SupplyUnderflow);
                    state.total_supply = state.total_supply
                        .checked_sub(burn)
                        .ok_or(RiftError::MathOverflow)?;
                    state.total_burned = state.total_burned
                        .checked_add(burn)
                        .ok_or(RiftError::MathOverflow)?;
                    emit!(BurnEvent { user: self.from_user.authority, amount: burn });
                }
                std::cmp::Ordering::Less => {
                    let mint = (-edge_cost) as u128;
                    state.total_supply = state.total_supply
                        .checked_add(mint)
                        .ok_or(RiftError::MathOverflow)?;
                    state.total_minted = state.total_minted
                        .checked_add(mint)
                        .ok_or(RiftError::MathOverflow)?;
                    emit!(MintEvent { user: self.from_user.authority, amount: mint });
                }
                _ => {}
            }
        }

        emit!(TransferEvent {
            from: self.from_user.authority,
            to: self.to_user.authority,
            amount,
        });

        state.check_invariant()
    }
}

// ============================================================================
// ACCOUNTS
// ============================================================================

#[account]
pub struct UserAccount {
    pub authority: Pubkey,
    pub base_balance: i128,
}
impl UserAccount { pub const SPACE: usize = 8 + 32 + 16; }

#[account]
pub struct EdgeAccount {
    pub weight: i128,
}
impl EdgeAccount { pub const SPACE: usize = 8 + 16; }

// ============================================================================
// CONTEXTS
// ============================================================================

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = CoreState::SPACE)]
    pub core_state: Account<'info, CoreState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(user: Pubkey)]
pub struct Register<'info> {
    #[account(mut, has_one = gate @ RiftError::UnauthorizedGate)]
    pub core_state: Account<'info, CoreState>,
    #[account(init, payer = gate, space = UserAccount::SPACE, seeds = [b"user", user.as_ref()], bump)]
    pub user_account: Account<'info, UserAccount>,
    #[account(mut)]
    pub gate: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unregister<'info> {
    #[account(mut, has_one = gate @ RiftError::UnauthorizedGate)]
    pub core_state: Account<'info, CoreState>,
    #[account(mut, close = gate, seeds = [b"user", user_account.authority.as_ref()], bump)]
    pub user_account: Account<'info, UserAccount>,
    #[account(mut)]
    pub gate: Signer<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    pub transfer_ctx: TransferCtx<'info>,
}

#[derive(Accounts)]
pub struct TransferWithEdge<'info> {
    pub transfer_ctx: TransferCtx<'info>,
    #[account(
        seeds = [b"edge", 
                 transfer_ctx.from_authority.key().as_ref(), 
                 transfer_ctx.to_authority.key().as_ref()],
        bump
    )]
    pub edge_account: Account<'info, EdgeAccount>,
}

#[derive(Accounts)]
#[instruction(from: Pubkey, to: Pubkey)]
pub struct SetEdge<'info> {
    #[account(has_one = gate @ RiftError::UnauthorizedGate)]
    pub core_state: Account<'info, CoreState>,
    #[account(
        init_if_needed,
        payer = gate,
        space = EdgeAccount::SPACE,
        seeds = [b"edge", from.as_ref(), to.as_ref()],
        bump
    )]
    pub edge_account: Account<'info, EdgeAccount>,
    #[account(mut)]
    pub gate: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Redistribute<'info> {
    #[account(mut, has_one = gate @ RiftError::UnauthorizedGate)]
    pub core_state: Account<'info, CoreState>,
    pub gate: Signer<'info>,
}

#[derive(Accounts)]
pub struct ApplyNegEntropy<'info> {
    #[account(mut, has_one = gate @ RiftError::UnauthorizedGate)]
    pub core_state: Account<'info, CoreState>,
    pub gate: Signer<'info>,
}

// ============================================================================
// ERRORS & EVENTS
// ============================================================================

#[error_code]
pub enum RiftError {
    #[msg("Critical: The core economic invariant has been violated.")]
    InvariantViolation,
    #[msg("Unauthorized: Caller is not the designated gate.")]
    UnauthorizedGate,
    #[msg("Operation denied: The protocol is currently paused.")]
    ProtocolPaused,
    #[msg("Capacity reached: Maximum number of participants exceeded.")]
    MaxParticipantsReached,
    #[msg("Transaction denied: The resulting balance exceeds the maximum allowable debt limit.")]
    DebtLimitExceeded,
    #[msg("State corruption: Attempted to burn more supply than currently exists.")]
    SupplyUnderflow,
    #[msg("Parameter out of bounds: The provided edge weight exceeds the protocol limits.")]
    EdgeLimitExceeded,
    #[msg("Operation invalid: The protocol currently has zero registered participants.")]
    ZeroParticipants,
    #[msg("Physical limit reached: Applying negative entropy would overflow the system bounds.")]
    PhysicalOverflowLimit,
    #[msg("Exit denied: Participants cannot unregister while holding a negative balance (debt).")]
    DebtOnExitNotAllowed,
    #[msg("Mathematical error: An arithmetic operation resulted in an overflow or underflow.")]
    MathOverflow,
    #[msg("Unauthorized: Invalid authority for target user.")]
    UnauthorizedAuthority,
}

#[event]
pub struct TransferEvent { pub from: Pubkey; pub to: Pubkey; pub amount: u128; }
#[event]
pub struct RedistributeEvent { pub amount: u128; pub per_user: u128; pub dust_retained: u128; }
#[event]
pub struct FieldUpdateEvent { pub new_global_field: i128; }
#[event]
pub struct RegisteredEvent { pub user: Pubkey; }
#[event]
pub struct UnregisteredEvent { pub user: Pubkey; }
#[event]
pub struct BurnEvent { pub user: Pubkey; pub amount: u128; }
#[event]
pub struct MintEvent { pub user: Pubkey; pub amount: u128; }