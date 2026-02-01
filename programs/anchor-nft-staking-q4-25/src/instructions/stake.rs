use anchor_lang::prelude::*;

#[cfg(not(feature = "no-mpl-core"))]
use mpl_core::{
    instructions::AddPluginV1CpiBuilder,
    types::{FreezeDelegate, Plugin, PluginAuthority},
    ID as CORE_PROGRAM_ID,
};

#[cfg(feature = "no-mpl-core")]
use solana_program::system_program::ID as CORE_PROGRAM_ID;

use crate::{
    errors::StakeError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: we are validdating from ownerrr
    #[account(
        mut,
        constraint = asset.owner == &CORE_PROGRAM_ID @ StakeError::InvalidAsset,
        constraint = !asset.data_is_empty() @ StakeError::AssetNotInitialized,
        constraint = asset.key() != collection.key() @ StakeError::InvalidCollection
    )]
    pub asset: UncheckedAccount<'info>,

    /// CHECK: same validdated throgh owner
    #[account(
        mut,
        constraint = collection.owner == &CORE_PROGRAM_ID @ StakeError::InvalidCollection,
        constraint = !collection.data_is_empty() @ StakeError::CollectionNotInitialized
    )]
    pub collection: UncheckedAccount<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + StakeAccount::INIT_SPACE,
        seeds = [b"stake", config.key().as_ref(), asset.key().as_ref()],
        bump
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
        bump = user_account.bump,
        constraint = user_account.amount_staked < config.max_stake @ StakeError::MaxStakeReached
    )]
    pub user_account: Account<'info, UserAccount>,

    /// CHECK: mpl-core program (mocked on localnet)
    #[account(address = CORE_PROGRAM_ID)]
    pub core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        self.stake_account.set_inner(StakeAccount {
            owner: self.user.key(),
            mint: self.asset.key(),
            staked_at: Clock::get()?.unix_timestamp,
            bump: bumps.stake_account,
        });

        self.user_account.amount_staked += 1;

        #[cfg(not(feature = "no-mpl-core"))]
        {
            let config_key = self.config.key();
            let asset_key = self.asset.key();
            let stake_account_key = self.stake_account.key();
            
            let signer_seeds: &[&[&[u8]]] = &[&[
                b"stake",
                config_key.as_ref(),
                asset_key.as_ref(),
                &[bumps.stake_account],
            ]];

            AddPluginV1CpiBuilder::new(&self.core_program.to_account_info())
                .asset(&self.asset.to_account_info())
                .collection(Some(&self.collection.to_account_info()))
                .payer(&self.user.to_account_info())
                .authority(Some(&self.user.to_account_info()))
                .system_program(&self.system_program.to_account_info())
                .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
                .init_authority(PluginAuthority::Address {
                    address: stake_account_key,
                })
                .invoke_signed(signer_seeds)?;
        }

        Ok(())
    }
}