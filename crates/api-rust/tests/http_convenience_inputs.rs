use bulk_client::{
    common::{side::Side, tif::TimeInForce},
    BulkHttpClient,
};
use serde_json::Value;
use solana_pubkey::Pubkey;
use std::{collections::HashMap, future::Future};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const NONCE: u64 = 424_242_424_242;

async fn capture_request<F, Fut>(call: F) -> Value
where
    F: FnOnce(BulkHttpClient, Pubkey) -> Fut,
    Fut: Future<Output = eyre::Result<bulk_client::msgs::responses::Response>>,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buf = [0; 4096];
        loop {
            let read = stream.read(&mut buf).await.unwrap();
            request.extend_from_slice(&buf[..read]);
            let Some(headers_end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap();
            if request.len() >= headers_end + 4 + content_length {
                let body = serde_json::from_slice(&request[headers_end + 4..]).unwrap();
                let response_body =
                    r#"{"response":{"data":{"statuses":[{"resting":{"oid":"test"}}]}}}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    response_body.len(), response_body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                return body;
            }
        }
    });

    let seed = bs58::encode([7_u8; 32]).into_string();
    let client = BulkHttpClient::with_url(&format!("http://{addr}"), Some(&seed)).unwrap();
    let account = Pubkey::new_from_array([9_u8; 32]);
    call(client, account).await.unwrap();
    server.await.unwrap()
}

fn assert_inputs(body: &Value, account: Pubkey) {
    assert_eq!(body["account"], account.to_string());
    assert_eq!(body["nonce"], NONCE);
}

#[tokio::test]
async fn trading_convenience_methods_preserve_explicit_transaction_inputs() {
    let body = capture_request(|client, account| async move {
        client
            .place_limit_order(
                "BTC-USD",
                Side::Buy,
                1.0,
                2.0,
                TimeInForce::GTC,
                false,
                Some(account),
                Some(NONCE),
            )
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .place_market_order(
                "BTC-USD",
                Side::Sell,
                2.0,
                false,
                Some(account),
                Some(NONCE),
            )
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .cancel_order(
                "BTC-USD",
                "11111111111111111111111111111111",
                Some(account),
                Some(NONCE),
            )
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client.cancel_all(vec![], Some(account), Some(NONCE)).await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));
}

#[tokio::test]
async fn account_convenience_methods_preserve_explicit_transaction_inputs() {
    let body = capture_request(|client, account| async move {
        client
            .update_leverage(
                HashMap::from([("BTC-USD".to_owned(), 2.0)]),
                Some(account),
                Some(NONCE),
            )
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .manage_agent_wallet(Pubkey::new_unique(), false, Some(account), Some(NONCE))
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .approve_builder_code(Pubkey::new_unique(), 1, Some(account), Some(NONCE))
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .revoke_builder_code(Pubkey::new_unique(), Some(account), Some(NONCE))
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));
}

#[tokio::test]
async fn faucet_convenience_methods_preserve_explicit_transaction_inputs() {
    let body = capture_request(|client, account| async move {
        client
            .whitelist_faucet(Pubkey::new_unique(), true, Some(account), Some(NONCE))
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));

    let body = capture_request(|client, account| async move {
        client
            .request_faucet(Some(account), None, Some(NONCE))
            .await
    })
    .await;
    assert_inputs(&body, Pubkey::new_from_array([9_u8; 32]));
}
