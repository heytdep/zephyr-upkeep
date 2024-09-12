use crate::*;

pub fn execute_transaction(env: &EnvClient, job: RunningJob) -> Result<u32, InternalError> {
    let contract = stellar_strkey::Contract::from_string(&job.contract)
        .unwrap()
        .0;
    let sequence = get_account_sequence(env, SOURCE_ACCOUNT);
    let args: soroban_sdk::Vec<Val> = soroban_sdk::vec![&env.soroban()];
    let tx = env.simulate_contract_call_to_tx(
        SOURCE_ACCOUNT.into(),
        sequence + 1,
        contract,
        Symbol::new(&env.soroban(), &job.function),
        args,
    );

    if tx.clone().unwrap().error.is_some() {
        Err(InternalError::Other)
    } else {
        let tx_to_sign =
            TransactionEnvelope::from_xdr_base64(tx.unwrap().tx.unwrap(), Limits::none()).unwrap();
        let inner_tx = match tx_to_sign {
            TransactionEnvelope::Tx(v1) => Some(v1.tx),
            _ => None,
        };
        let end_fee = if let Some(inner_tx) = inner_tx {
            let end_fee = inner_tx.fee.clone();
            if end_fee > job.balance {
                return Err(InternalError::NoBalanceLeft);
            }
            sign_and_send_transaction(env, inner_tx);
            end_fee
        } else {
            0
        };

        Ok(end_fee)
    }
}

pub fn repay_balance(env: &EnvClient, job: RunningJob) {
    let source_bytes = account_str_to_bytes(&SOURCE_ACCOUNT);
    let dest_bytes = account_str_to_bytes(&job.creator);
    let source_sequence = get_account_sequence(env, SOURCE_ACCOUNT);
    let inner_tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(source_bytes)),
        fee: 200,
        seq_num: soroban_sdk::xdr::SequenceNumber(source_sequence + 1),
        cond: soroban_sdk::xdr::Preconditions::None,
        memo: Memo::None,
        ext: TransactionExt::V0,
        operations: std::vec![Operation {
            source_account: None,
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256(dest_bytes)),
                asset: soroban_sdk::xdr::Asset::Native,
                amount: job.balance as i64 - 200,
            }),
        }]
        .try_into()
        .unwrap(),
    };
    sign_and_send_transaction(env, inner_tx);
}
