#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();
use multiversx_sc::types::BigUint;

pub const REWARDS_PER_BLOCK: u64 = 3_000_000_000;

#[derive(TypeAbi, TopEncode, TopDecode, PartialEq, Debug)]
pub struct StakingPosition<M: ManagedTypeApi> {
    pub stake_amount: BigUint<M>,
    pub last_action_block: u64,    
}

#[multiversx_sc::contract]
pub trait StakingContract {
    #[init]
    fn init(&self) {
        self.total_rewards().set(&BigUint::zero());
        let current_timestamp = self.blockchain().get_block_timestamp();
        self.contract_creation_timestamp().set(current_timestamp);                
    }

    #[payable("EGLD")]
    #[endpoint]
    fn stake(&self) {
        let payment_amount = self.call_value().egld_value().clone_value();
        require!(payment_amount > 0, "Must pay more than 0");

        let caller = self.blockchain().get_caller();
        let stake_mapper = self.staking_position(&caller);

        let new_user = self.staked_addresses().insert(caller.clone());
        let mut staking_pos = if !new_user {
            stake_mapper.get()
        } else {
            let current_block = self.blockchain().get_block_epoch();
            StakingPosition {
                stake_amount: BigUint::zero(),
                last_action_block: current_block,                
            }
        };

        self.claim_rewards_for_user(&caller, &mut staking_pos);
        staking_pos.stake_amount += payment_amount;

        stake_mapper.set(&staking_pos);
    }

    #[endpoint]
    fn unstake(&self, opt_unstake_amount: OptionalValue<BigUint>) {
        let caller = self.blockchain().get_caller();
        self.require_user_staked(&caller);

        let stake_mapper = self.staking_position(&caller);
        let mut staking_pos = stake_mapper.get();

        let unstake_amount = match opt_unstake_amount {
            OptionalValue::Some(amt) => amt,
            OptionalValue::None => staking_pos.stake_amount.clone(),
        };
        require!(
            unstake_amount > 0 && unstake_amount <= staking_pos.stake_amount,
            "Invalid unstake amount"
        );

        self.claim_rewards_for_user(&caller, &mut staking_pos);
        staking_pos.stake_amount -= &unstake_amount;

        if staking_pos.stake_amount > 0 {
            stake_mapper.set(&staking_pos);
        } else {
            stake_mapper.clear();
            self.staked_addresses().swap_remove(&caller);
        }

        self.send().direct_egld(&caller, &unstake_amount);
    }

    #[endpoint(claim_rewards)]
    fn claim_rewards(&self) {
        let caller = self.blockchain().get_caller();
        self.require_user_staked(&caller);

        let stake_mapper = self.staking_position(&caller);
        let mut staking_pos = stake_mapper.get();
        
        self.claim_rewards_for_user(&caller, &mut staking_pos);

        stake_mapper.set(&staking_pos);
    }

    fn require_user_staked(&self, user: &ManagedAddress) {
        require!(self.staked_addresses().contains(user), "Must stake first");
    }

    fn claim_rewards_for_user(
        &self,
        user: &ManagedAddress,
        staking_pos: &mut StakingPosition<Self::Api>,
    ) {
        let reward_amount = self.update_rewards(staking_pos);
        let current_block = self.blockchain().get_block_nonce();
        staking_pos.last_action_block = current_block;
    
        if reward_amount > 0 {            
            self.send().direct_egld(user, &reward_amount);
        }
    }

    fn update_total_rewards(&self) -> BigUint {
        let current_timestamp = self.blockchain().get_block_timestamp();
        let total_timestamp_passed = current_timestamp - self.contract_creation_timestamp().get();
        let total_rewards = BigUint::from(total_timestamp_passed) * BigUint::from(REWARDS_PER_BLOCK);

        self.total_rewards().set(&total_rewards);  

        total_rewards
    }

    fn update_rewards(&self, staking_position: &StakingPosition<Self::Api>) -> BigUint {
        let current_block = self.blockchain().get_block_nonce();
        if current_block <= staking_position.last_action_block {
            return BigUint::zero();
        }
        
        let block_diff = current_block - staking_position.last_action_block;
        let total_staked = self.get_contract_balance();

        &staking_position.stake_amount * REWARDS_PER_BLOCK * block_diff / total_staked
    }  

    #[view(calculateRewardsForUser)]
    fn calculate_rewards_for_user(&self, addr: ManagedAddress) -> BigUint {
        let staking_pos = self.staking_position(&addr).get();
        self.update_rewards(&staking_pos)
    }   

    #[view(getContractBalance)]
    fn get_contract_balance(&self) -> BigUint {
        let contract_balance = self.blockchain().get_sc_balance(&EgldOrEsdtTokenIdentifier::egld(), 0);
        BigUint::from(contract_balance)
    }    

    #[view(contractCreationBlock)]
    fn contract_creation_block(&self) -> u64 {
        self.blockchain().get_block_epoch()
    }

    #[view(contractCreationTimestamp)]
    #[storage_mapper("creationTimestamp")]
    fn contract_creation_timestamp(&self) -> SingleValueMapper<u64>;
    
    #[view(getStakeAmount)]
    fn get_stake_amount(&self, user: &ManagedAddress) -> BigUint {
        let stake_mapper = self.staking_position(user);
        let staking_pos = stake_mapper.get();

        staking_pos.stake_amount.clone()
    }   

    #[view(getUpdatedTotalRewards)]
    fn get_updated_total_rewards(&self) -> BigUint {
        self.update_total_rewards()
    }

    #[view(getStakedAddresses)]
    #[storage_mapper("stakedAddresses")]
    fn staked_addresses(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getStakingPosition)]
    #[storage_mapper("stakingPosition")]
    fn staking_position(
        &self,
        addr: &ManagedAddress,
    ) -> SingleValueMapper<StakingPosition<Self::Api>>;

    #[storage_mapper("totalRewards")]
    fn total_rewards(&self) -> SingleValueMapper<BigUint>;      
}