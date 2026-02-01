use anchor_lang::prelude::*;

#[cfg(not(feature = "no-mpl-core"))]
use mpl_core::{
    instructions::{RemovePluginV1CpiBuilder, UpdatePluginV1CpiBuilder},
    types::{FreezeDelegate, Plugin, PluginType},
    ID as CORE_PROGRAM_ID,
};

#[cfg(feature = "no-mpl-core")]
use solana_program::system_program::ID as CORE_PROGRAM_ID;

use crate::{
    errors::StakeError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: validattt with owner and data
    #[account(
        mut,
        constraint = asset.owner == &CORE_PROGRAM_ID @ StakeError::InvalidAsset,
        constraint = !asset.data_is_empty() @ StakeError::AssetNotInitialized,
        constraint = asset.key() != collection.key() @ StakeError::InvalidCollection
    )]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: some data constraints
    #[account(
        mut,
        constraint = collection.owner == &CORE_PROGRAM_ID @ StakeError::InvalidCollection,
        constraint = !collection.data_is_empty() @ StakeError::CollectionNotInitialized
    )]
    pub collection: UncheckedAccount<'info>,

    #[account(
        mut,
        close = user,
        seeds = [b"stake", config.key().as_ref(), asset.key().as_ref()],
        bump = stake_account.bump,
        constraint = stake_account.owner == user.key() @ StakeError::NotOwner
    )]
    pub stake_account: Account<'info, StakeAccount>,

    #[account(
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, StakeConfig>,

    #[account(
        mut,
        seeds = [b"user", user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,

    /// CHECK: mpl-core program (mocked on localnet)
    #[account(address = CORE_PROGRAM_ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Unstake<'info> {
    pub fn unstake(&mut self) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        let time_elapsed = current_time - self.stake_account.staked_at;
        let freeze_period_seconds = (self.config.freeze_period as i64) * 86400;

        require!(
            time_elapsed >= freeze_period_seconds,
            StakeError::FreezePeriodNotPassed
        );

        let points_earned =
            ((time_elapsed / 86400) as u32) * (self.config.points_per_stake as u32);
        self.user_account.points += points_earned;
        self.user_account.amount_staked -= 1;

        #[cfg(not(feature = "no-mpl-core"))]
        {
            let config_key = self.config.key();
            let asset_key = self.asset.key();
            
            let signer_seeds: &[&[&[u8]]] = &[&[
                b"stake",
                config_key.as_ref(),
                asset_key.as_ref(),
                &[self.stake_account.bump],
            ]];

            UpdatePluginV1CpiBuilder::new(&self.core_program.to_account_info())
                .asset(&self.asset.to_account_info())
                .collection(Some(&self.collection.to_account_info()))
                .payer(&self.user.to_account_info())
                .authority(Some(&self.stake_account.to_account_info()))
                .system_program(&self.system_program.to_account_info())
                .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
                .invoke_signed(signer_seeds)?;

            RemovePluginV1CpiBuilder::new(&self.core_program.to_account_info())
                .asset(&self.asset.to_account_info())
                .collection(Some(&self.collection.to_account_info()))
                .payer(&self.user.to_account_info())
                .authority(Some(&self.stake_account.to_account_info()))
                .system_program(&self.system_program.to_account_info())
                .plugin_type(PluginType::FreezeDelegate)
                .invoke_signed(signer_seeds)?;
        }

        Ok(())
    }
}