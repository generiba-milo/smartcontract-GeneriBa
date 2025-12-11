use anchor_lang::prelude::*;

declare_id!("FSFSmPKior2TJoEwMALubV5iMtSusyTXSN7tUBGnqRQp"); // replace with your program id

#[program]
pub mod simple_escrow {
    use super::*;

    /// Create escrow and deposit funds into the newly created escrow account
    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        recipient: Pubkey,
        amount: u64,
    ) -> Result<()> {
        // initialize state
        let escrow = &mut ctx.accounts.escrow;
        escrow.initializer = ctx.accounts.initializer.key();
        escrow.recipient = recipient;
        escrow.amount = amount;
        escrow.released = false;

        // transfer lamports from initializer to escrow account
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.initializer.key(),
            &ctx.accounts.escrow.key(),
            amount,
        );

        let account_infos = &[
            ctx.accounts.initializer.to_account_info(),
            ctx.accounts.escrow.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ];

        anchor_lang::solana_program::program::invoke(&ix, account_infos)?;

        Ok(())
    }

    /// Release funds to recipient (only initializer can call)
    pub fn release(ctx: Context<Release>) -> Result<()> {
        // Read immutable data first
        let escrow_state = &ctx.accounts.escrow;
        require!(!escrow_state.released, EscrowError::AlreadyReleased);
        require_keys_eq!(escrow_state.initializer, ctx.accounts.initializer.key(), EscrowError::Unauthorized);

        let amount = escrow_state.amount;
        // drop immutable reference by not holding &mut later (we used &ctx.accounts.escrow above)
        // Now get owned AccountInfo clones for mutable lamport ops
        let escrow_info = ctx.accounts.escrow.to_account_info().clone();
        let recipient_info = ctx.accounts.recipient.to_account_info().clone();

        // perform lamport transfer (mutably borrow lamports on the clones)
        **escrow_info.try_borrow_mut_lamports()? = escrow_info
            .lamports()
            .checked_sub(amount)
            .ok_or(EscrowError::InsufficientFunds)?;
        **recipient_info.try_borrow_mut_lamports()? = recipient_info
            .lamports()
            .checked_add(amount)
            .ok_or(EscrowError::LamportOverflow)?;

        // Now mutate the state in the account struct
        let escrow_mut = &mut ctx.accounts.escrow;
        escrow_mut.released = true;

        Ok(())
    }

    /// Cancel escrow and refund initializer (only initializer can call) - then close the escrow account
    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        // Read immutable state first
        let escrow_state = &ctx.accounts.escrow;
        require!(!escrow_state.released, EscrowError::AlreadyReleased);
        require_keys_eq!(escrow_state.initializer, ctx.accounts.initializer.key(), EscrowError::Unauthorized);

        let amount = escrow_state.amount;

        // Owned AccountInfo clones for mutable lamport ops
        let escrow_info = ctx.accounts.escrow.to_account_info().clone();
        let initializer_info = ctx.accounts.initializer.to_account_info().clone();

        // Refund lamports from escrow to initializer
        **escrow_info.try_borrow_mut_lamports()? = escrow_info
            .lamports()
            .checked_sub(amount)
            .ok_or(EscrowError::InsufficientFunds)?;
        **initializer_info.try_borrow_mut_lamports()? = initializer_info
            .lamports()
            .checked_add(amount)
            .ok_or(EscrowError::LamportOverflow)?;

        // mark released so state won't be reused (optional)
        let escrow_mut = &mut ctx.accounts.escrow;
        escrow_mut.released = true;

        // The `#[account(..., close = initializer)]` attribute will cause Anchor to close the escrow account
        // and transfer any remaining rent-exempt lamports to the initializer after instruction finishes.
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(recipient: Pubkey, amount: u64)]
pub struct CreateEscrow<'info> {
    /// Program-owned escrow account (will hold state and lamports)
    #[account(
        init,
        payer = initializer,
        space = 8 + 32 + 32 + 8 + 1, // discriminator + initializer + recipient + amount + released
    )]
    pub escrow: Account<'info, EscrowState>,

    #[account(mut)]
    pub initializer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    #[account(mut)]
    pub escrow: Account<'info, EscrowState>,

    /// CHECK: recipient can be any system account
    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    pub initializer: Signer<'info>,
}

#[derive(Accounts)]
pub struct Cancel<'info> {
    /// close = initializer will allow Anchor to reclaim the account's data and send remaining lamports to initializer
    #[account(mut, close = initializer)]
    pub escrow: Account<'info, EscrowState>,

    #[account(mut)]
    pub initializer: Signer<'info>,
}

#[account]
pub struct EscrowState {
    pub initializer: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
    pub released: bool,
}

#[error_code]
pub enum EscrowError {
    #[msg("Unauthorized action")]
    Unauthorized,
    #[msg("Escrow already released")]
    AlreadyReleased,
    #[msg("Insufficient funds in escrow")]
    InsufficientFunds,
    #[msg("Lamport arithmetic overflow")]
    LamportOverflow,
}
