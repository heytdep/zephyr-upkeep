use crate::*;

pub fn create_agnostic_request(tx: &str) -> AgnosticRequest {
    AgnosticRequest {
        body: Some(format!("tx={}", encode(tx))),
        url: "https://horizon-testnet.stellar.org/transactions".to_string(),
        method: zephyr_sdk::Method::Post,
        headers: std::vec![(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string()
        )],
    }
}

pub fn sign_and_send_transaction(env: &EnvClient, tx: Transaction) {
    let signed = sign_transaction(tx, &NETWORK, &SECRET);
    env.send_web_request(create_agnostic_request(&signed));
}

pub fn account_str_to_bytes(account: &str) -> [u8; 32] {
    stellar_strkey::ed25519::PublicKey::from_string(account)
        .unwrap()
        .0
}

pub fn get_account_sequence(env: &EnvClient, account: &str) -> i64 {
    let account_bytes = account_str_to_bytes(account);
    env.read_account_from_ledger(account_bytes)
        .unwrap()
        .unwrap()
        .seq_num as i64
}

pub fn update_job(env: &EnvClient, job: &RunningJob) {
    let _ = env
        .update()
        .column_equal_to("name", job.name.clone())
        .execute(job);
}

pub fn job_by_name(env: &EnvClient, name: String) -> Option<RunningJob> {
    let read = env
        .read_filter()
        .column_equal_to("name", name)
        .read()
        .unwrap();
    read.first().cloned()
}
