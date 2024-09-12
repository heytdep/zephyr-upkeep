#[macro_use]
extern crate dotenv_codegen;

mod tx;
mod utils;
use serde::Deserialize;
use tx::{execute_transaction, repay_balance};
use urlencoding::encode;
use utils::*;
use zephyr_sdk::{
    prelude::*,
    soroban_sdk::{
        self,
        xdr::{
            Asset, Memo, MuxedAccount, Operation, OperationBody, PaymentOp, Transaction,
            TransactionEnvelope, TransactionExt, Uint256,
        },
        Symbol, Val,
    },
    utils::sign_transaction,
    AgnosticRequest, DatabaseDerive, EnvClient,
};

const NETWORK: &'static str = dotenv!("NETWORK");
const SOURCE_ACCOUNT: &'static str = dotenv!("SOURCE_ACCOUNT");
const SECRET: &'static str = dotenv!("SECRET");

#[derive(Debug)]
pub enum InternalError {
    NoBalanceLeft,
    Other,
}

#[derive(DatabaseDerive, Clone)]
#[with_name("jobs")]
pub struct RunningJob {
    name: String,
    interval: u64,
    function: String,
    contract: String,
    last: u64,
    balance: u32,
    creator: String,
}

#[derive(Deserialize)]
pub struct Request {
    name: String,
    interval: u64,
    function: String,
    contract: String,
    counter_start: u64,
    creator: String,
}

#[no_mangle]
pub extern "C" fn newjob() {
    let env = EnvClient::empty();
    let request: Request = env.read_request_body();
    let name = request.name;
    let job = job_by_name(&env, name.clone());
    if job.is_some() {
        env.conclude("Name already exists")
    } else {
        let job = RunningJob {
            balance: 0,
            last: request.counter_start,
            name,
            interval: request.interval,
            function: request.function,
            contract: request.contract,
            creator: request.creator,
        };
        env.put(&job)
    }
}

#[no_mangle]
pub extern "C" fn on_close() {
    let env = EnvClient::new();
    let jobs: Vec<RunningJob> = env.read();
    let time = env.reader().ledger_timestamp();
    for (tx, meta) in env.reader().envelopes_with_meta() {
        if let soroban_sdk::xdr::TransactionResultResult::TxSuccess(_) = meta.result.result.result {
            match tx {
                TransactionEnvelope::Tx(v1) => {
                    let source = v1.tx.source_account.clone();
                    let mut repay = false;
                    let memo = match v1.tx.memo.clone() {
                        Memo::Text(name) => Some(name.to_string()),
                        _ => None,
                    };
                    if let Some(mut memo) = memo {
                        if let Some(no_delete_memo) = memo.strip_prefix("d:") {
                            repay = true;
                            memo = no_delete_memo.into();
                        }
                        for op in v1.tx.operations.to_vec() {
                            match op.body {
                                OperationBody::Payment(payment) => {
                                    if let MuxedAccount::Ed25519(Uint256(bytes)) =
                                        payment.destination
                                    {
                                        if stellar_strkey::ed25519::PublicKey(bytes).to_string()
                                            == SOURCE_ACCOUNT
                                            && payment.asset == Asset::Native
                                        {
                                            let job = job_by_name(&env, memo.clone());

                                            if let Some(mut job) = job {
                                                if repay {
                                                    if source
                                                        == MuxedAccount::Ed25519(Uint256(
                                                            account_str_to_bytes(&job.creator),
                                                        ))
                                                    {
                                                        repay_balance(&env, job.clone());
                                                        job.balance = 0;
                                                        update_job(&env, &job);
                                                    }
                                                } else {
                                                    let refill = payment.amount;
                                                    job.balance += refill as u32;
                                                    update_job(&env, &job);
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => (), // not supported as payment method for now
                            }
                        }
                    }
                }
                _ => (), // not supported as payment method for now
            }
        }
    }

    for mut job in jobs {
        if job.last + job.interval <= time {
            let action = execute_transaction(&env, job.clone());
            if action.is_ok() {
                job.balance -= action.unwrap();
                job.last = time;
                update_job(&env, &job);
            }
        };
    }
}
