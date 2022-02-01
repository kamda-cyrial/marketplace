use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    msg,
    pubkey::Pubkey,
    program_error::ProgramError,
    system_instruction,
    sysvar::{rent::Rent, Sysvar}, borsh::try_from_slice_unchecked,
};
use spl_associated_token_account::create_associated_token_account;
use spl_token::instruction::*;


use metaplex_token_metadata::{id, state::{Metadata}};

entrypoint!(process_instructions);


enum InstructionEnum{
    CreateCollection,
    CreateLimitOrder{
        price: u32
    },
    CloseLimitOrder,
    FillLimitOrder,
}


#[derive(BorshSerialize, BorshDeserialize)]
struct CollectionData{
    address: Pubkey,
    min_listed: u32,
    max_listed: u32,
    max_ever: u32,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct ContainerData{
    collection_address: Pubkey,
    mint_address: Pubkey,
    price: u32,
    owner: Pubkey,
    state: bool,
}

impl InstructionEnum{
    fn decode_instrction(data: &[u8]) -> Result<Self, ProgramError> {
       Ok(match data[0]{
            0 => {
                
                Self::CreateCollection
            }
            1 => {
                let price = ((data[1] as f32 *  data[2] as f32 + data[3] as f32  + (data[4] as f32 * data[5] as f32 + data[6] as f32) / 10000.0) * (10.0 as f32).powf(9.0) as f32) as u32;
                Self::CreateLimitOrder{price:price}
            }
            2 => {
                Self::CloseLimitOrder
            }
            3 => {
                Self::FillLimitOrder
            }
            _ => Err(ProgramError::InvalidInstructionData)?
        })

    }
}

fn create_collection(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult{
    let account_info_iter = &mut accounts.iter();


    let payer_account_info = next_account_info(account_info_iter)?;
    let collection_account_info = next_account_info(account_info_iter)?;
    let collection_pda_info = next_account_info(account_info_iter)?;

    let (collection_pda, collection_pda_bump) = Pubkey::find_program_address(&[b"Gamestree_seed", &collection_account_info.key.to_bytes()], program_id);

    if collection_pda != *collection_pda_info.key{
        Err(ProgramError::InvalidAccountData)?
    }
    let space = 50;
    let lamports = Rent::get()?.minimum_balance(space as usize);


    invoke_signed(
        &system_instruction::create_account(&payer_account_info.key, &collection_pda, lamports, space, &program_id),
        &[payer_account_info.clone(), collection_pda_info.clone()],
        &[&[b"Gamestree_seed", &collection_account_info.key.to_bytes(), &[collection_pda_bump]]]
    )?;

    let collection_data = CollectionData{
        address: *collection_account_info.key,
        min_listed: 0,
        max_listed: 0,
        max_ever: 0,
    };

    collection_data.serialize(&mut &mut collection_pda_info.data.borrow_mut()[..])?;


    Ok(())
}

fn create_limit_order(program_id: &Pubkey, accounts: &[AccountInfo], price: u32) -> ProgramResult{

    let account_info_iter = &mut accounts.iter();

    let payer_account_info = next_account_info(account_info_iter)?;
    let collection_account_info = next_account_info(account_info_iter)?;
    let collection_pda_info = next_account_info(account_info_iter)?;
    let container_account_info = next_account_info(account_info_iter)?;
    let mint_account_info = next_account_info(account_info_iter)?;
    let associated_account_info = next_account_info(account_info_iter)?;
    let rent_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let system_account_info = next_account_info(account_info_iter)?;
    let current_associated_account_info = next_account_info(account_info_iter)?;
    let metadata_account_info = next_account_info(account_info_iter)?;

    let collection_data: CollectionData = try_from_slice_unchecked(&collection_pda_info.data.borrow())?;

    let collection_unique_bump = collection_data.max_listed;
    let (container_pda, container_pda_bump) = Pubkey::find_program_address( &[b"Gamestree_seed", &collection_unique_bump.to_be_bytes(), &collection_account_info.key.to_bytes()], program_id);
    let container_seed: &[&[u8]] = &[b"Gamestree_seed", &collection_unique_bump.to_be_bytes(), &collection_account_info.key.to_bytes(), &[container_pda_bump]];

    if *container_account_info.key != container_pda{
        Err(ProgramError::InvalidAccountData)?
    }

    if collection_data.max_listed >= collection_data.max_ever{
        let space = 50;
        let lamports = Rent::get()?.minimum_balance(space as usize);
        invoke_signed(
            &system_instruction::create_account(&payer_account_info.key, &container_pda, lamports, space, &program_id),
            &[payer_account_info.clone(), container_account_info.clone()],
            &[container_seed]
        )?;
    }
    else{
        let current_container_data: ContainerData = try_from_slice_unchecked(&collection_pda_info.data.borrow())?;
        if current_container_data.state == true{
            msg!("current Container doesn't seem to be Empty, Big Problem");
            Err(ProgramError::InvalidSeeds)?
        }
    }
    let new_container_data = ContainerData{
        collection_address: *collection_account_info.key,
        mint_address: *mint_account_info.key,
        price: price,
        owner: *payer_account_info.key,
        state: true
    };

    new_container_data.serialize(&mut &mut container_account_info.data.borrow_mut()[..])?;
    

    let (metadata_pda, _metadata_nonce) = Pubkey::find_program_address(&[b"metadata", &id().to_bytes(), &mint_account_info.key.to_bytes()], &id());

    if *metadata_account_info.key != metadata_pda{
        Err(ProgramError::InvalidAccountData)?
    }

    let metadata = Metadata::from_account_info(metadata_account_info)?;

    match metadata.data.creators{
        Some(creators) =>{
            for creator in creators.iter(){
                if &creator.address == collection_account_info.key{
                    if creator.verified{
                        break;
                    }
                    else{
                        msg!("NFT, Not signed by Creator");
                        Err(ProgramError::InvalidAccountData)?
                    }
                }
            }
            msg!("NFT, Wrong Creator in Account Sent");
            Err(ProgramError::InvalidAccountData)?
        }
        None => {msg!("Cannot Certify Authenticity of this NFT"); Err(ProgramError::InvalidAccountData)?}
    }


    invoke(
        &create_associated_token_account(
            payer_account_info.key,
            container_account_info.key,
            mint_account_info.key,
        ),
        &[
            payer_account_info.clone(),
            associated_account_info.clone(),
            container_account_info.clone(),
            mint_account_info.clone(),
            system_account_info.clone(),
            token_program_info.clone(),
            rent_account_info.clone(),
        ],
    )?;

    invoke(
        &transfer(token_program_info.key, current_associated_account_info.key, associated_account_info.key, payer_account_info.key, &[], 1)?, //don't know what authority_pubkey is, it could be mint_authority, but I am not sure
        &[
            current_associated_account_info.clone(),
            associated_account_info.clone(),
            payer_account_info.clone(),
        ]
    )?;

    Ok(())
}


fn process_instructions(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult{
    let instruction  = InstructionEnum::decode_instrction(instruction_data)?;
 
    match instruction{
        InstructionEnum::CreateCollection => {
           create_collection(program_id, accounts)?
        }
        InstructionEnum::CreateLimitOrder{price} => {
            create_limit_order(program_id, accounts, price)?
        }
        InstructionEnum::CloseLimitOrder => {
            ()
        }
        InstructionEnum::FillLimitOrder => {
            ()
        }


        // _ => Err(ProgramError::InvalidInstructionData)?
    }


    Ok(())
}






// #[cfg(test)]
// mod tests {
//     #[test]
//     fn it_works() {
//         let result = 2 + 2;
//         assert_eq!(result, 4);
//     }
// }
