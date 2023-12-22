use multiversx_sc::{codec::multi_types::OptionalValue, types::Address};
use multiversx_sc_scenario::{
    managed_address, managed_biguint, rust_biguint, whitebox_legacy::*, DebugApi,
};
use staking_contract::*;
//use multiversx_sc_scenario::num_bigint::BigUint;
use multiversx_sc::types::BigUint;


const WASM_PATH: &'static str = "output/staking-contract.wasm";
const USER_BALANCE: u64 = 1_000_000_000_000_000_000;
const BLOCKS_PASSED: u64 = 100;

struct ContractSetup<ContractObjBuilder>
where
    ContractObjBuilder: 'static + Copy + Fn() -> staking_contract::ContractObj<DebugApi>,
{
    pub b_mock: BlockchainStateWrapper,
    pub owner_address: Address,
    pub user_address: Address,
    pub contract_wrapper:
        ContractObjWrapper<staking_contract::ContractObj<DebugApi>, ContractObjBuilder>,
}

impl<ContractObjBuilder> ContractSetup<ContractObjBuilder>
where
    ContractObjBuilder: 'static + Copy + Fn() -> staking_contract::ContractObj<DebugApi>,
{
    pub fn new(sc_builder: ContractObjBuilder) -> Self {
        let rust_zero = rust_biguint!(0u64);
        let mut b_mock = BlockchainStateWrapper::new();
        let owner_address = b_mock.create_user_account(&rust_zero);
        let user_address = b_mock.create_user_account(&rust_biguint!(USER_BALANCE));
        let sc_wrapper =
            b_mock.create_sc_account(&rust_zero, Some(&owner_address), sc_builder, WASM_PATH);

        // simulate deploy
        b_mock
            .execute_tx(&owner_address, &sc_wrapper, &rust_zero, |sc| {
                sc.init();
            })
            .assert_ok();

        ContractSetup {
            b_mock,
            owner_address,
            user_address,
            contract_wrapper: sc_wrapper,
        }
    }
}

use multiversx_sc::contract_base::ContractBase;
use staking_contract::endpoints::staking_position;

#[test]
fn stake_unstake_test() {
    let mut setup = ContractSetup::new(staking_contract::contract_obj);
    let user_addr = setup.user_address.clone();

    setup
        .b_mock
        .check_egld_balance(&user_addr, &rust_biguint!(USER_BALANCE));
    setup
        .b_mock
        .check_egld_balance(setup.contract_wrapper.address_ref(), &rust_biguint!(0));

    // stake full
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(USER_BALANCE),
            |sc| {
                sc.stake();

                assert_eq!(
                    sc.staking_position(&managed_address!(&user_addr)).get(),
                    StakingPosition {
                        stake_amount: managed_biguint!(USER_BALANCE),
                        last_action_block: 0,                        
                    }
                );
            },
        )
        .assert_ok();

    setup
        .b_mock
        .check_egld_balance(&user_addr, &rust_biguint!(0));
    setup.b_mock.check_egld_balance(
        setup.contract_wrapper.address_ref(),
        &rust_biguint!(USER_BALANCE),
    );

    // unstake partial
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(0),
            |sc| {
                sc.unstake(OptionalValue::Some(managed_biguint!(USER_BALANCE / 2)));

                assert_eq!(
                    sc.staking_position(&managed_address!(&user_addr)).get(),
                    StakingPosition {
                        stake_amount: managed_biguint!(USER_BALANCE / 2),
                        last_action_block: 0
                    }
                );
            },
        )
        .assert_ok();

    // unstake full
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(0),
            |sc| {
                sc.unstake(OptionalValue::None);

                assert!(sc
                    .staking_position(&managed_address!(&user_addr))
                    .is_empty());
            },
        )
        .assert_ok();

    setup
        .b_mock
        .check_egld_balance(&user_addr, &rust_biguint!(USER_BALANCE));
    setup
        .b_mock
        .check_egld_balance(setup.contract_wrapper.address_ref(), &rust_biguint!(0));

    
}

#[test]
fn rewards_test() {
    let mut setup = ContractSetup::new(staking_contract::contract_obj);
    let user_addr = setup.user_address.clone();

    // stake full
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(USER_BALANCE),
            |sc| {
                sc.stake();

                assert_eq!(
                    sc.staking_position(&managed_address!(&user_addr)).get(),
                    StakingPosition {
                        stake_amount: managed_biguint!(USER_BALANCE),
                        last_action_block: 0
                    }
                );
            },
        )
        .assert_ok();

    setup.b_mock.set_block_nonce(BLOCKS_PASSED);

    // query rewards
    setup
        .b_mock
        .execute_query(&setup.contract_wrapper, |sc| {
            let actual_rewards = sc.calculate_rewards_for_user(managed_address!(&user_addr));
            let staking_pos = sc.staking_position(&managed_address!(&user_addr)).get();
            let total_staked = &staking_pos.stake_amount;
            let expected_rewards = &staking_pos.stake_amount * REWARDS_PER_BLOCK * BLOCKS_PASSED / total_staked;
            assert_eq!(actual_rewards, expected_rewards);
        })
        .assert_ok();

    // claim rewards
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(0),
            |sc| {
                assert_eq!(
                    sc.staking_position(&managed_address!(&user_addr)).get(),
                    StakingPosition {
                        stake_amount: managed_biguint!(USER_BALANCE),
                        last_action_block: 0
                    }
                );

                sc.claim_rewards();

                assert_eq!(
                    sc.staking_position(&managed_address!(&user_addr)).get(),
                    StakingPosition {
                        stake_amount: managed_biguint!(USER_BALANCE),
                        last_action_block: BLOCKS_PASSED
                    }
                );
            },
        )
        .assert_ok();  

    setup.b_mock.check_egld_balance(
        &user_addr,
        &rust_biguint!(REWARDS_PER_BLOCK * BLOCKS_PASSED),
    ); 

    // query rewards after claim
    setup
        .b_mock
        .execute_query(&setup.contract_wrapper, |sc| {
            let actual_rewards = sc.calculate_rewards_for_user(managed_address!(&user_addr));
            let expected_rewards = managed_biguint!(0);
            assert_eq!(actual_rewards, expected_rewards);
        })
        .assert_ok();
}



