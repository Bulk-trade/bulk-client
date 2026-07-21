use {
    bulk_client::{
        msgs::{
            AccountActivity, ClosedPosition, FundingPayment, HistoryCoverageStatus, HistoryFill,
            HistoryHttpError, HistoryPage, HistoryQuery, HistoryTrigger, RiskEvent, RiskEventType,
            TerminalOrder,
        },
        BulkHttpClient,
    },
    reqwest::StatusCode,
    serde_json::{json, Value},
    tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    },
};

const PUBKEY: &str = "11111111111111111111111111111111";

async fn spawn_json_server(status: &str, body: Value) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fixture server");
    let address = listener.local_addr().expect("fixture address");
    let (request_tx, request_rx) = oneshot::channel();
    let status = status.to_string();
    let body = body.to_string();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept fixture request");
        let mut request = Vec::new();
        loop {
            let mut bytes = [0; 1024];
            let read = stream.read(&mut bytes).await.expect("read fixture request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&bytes[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        request_tx
            .send(String::from_utf8(request).expect("request is UTF-8"))
            .expect("send captured request");
        stream
            .write_all(
                format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("write fixture response");
    });
    (format!("http://{address}"), request_rx)
}

fn page(row: Value) -> Value {
    json!({
        "data": [row],
        "page": {
            "nextCursor": "next_page",
            "hasMore": true,
            "asOfSlot": u64::MAX,
            "startSlot": 9_007_199_254_740_993_u64,
            "endSlot": u64::MAX,
            "coverage": "complete",
            "minAvailableSlot": 9_007_199_254_740_993_u64
        }
    })
}

fn fill_row() -> Value {
    json!({
        "maker": PUBKEY,
        "taker": PUBKEY,
        "orderIdMaker": PUBKEY,
        "orderIdTaker": PUBKEY,
        "isBuy": true,
        "symbol": "BTC-USD",
        "amount": 1.25,
        "price": 100_000.0,
        "makerFee": 1.0,
        "takerFee": 2.0,
        "fee": 1.0,
        "reasonCode": 3,
        "iso": true,
        "isoPubkey": PUBKEY,
        "reason": "matched",
        "counterpartyHint": "1111..1111",
        "slot": u64::MAX,
        "timestamp": u64::MAX - 1,
        "sequence": u64::MAX - 2
    })
}

fn position_row() -> Value {
    json!({
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "quantity": -1.25,
        "maxQuantity": -2.0,
        "totalVolume": 3.0,
        "avgOpenPrice": 90_000.0,
        "avgClosePrice": 100_000.0,
        "realizedPnl": 12_500.0,
        "fees": 12.0,
        "funding": -2.0,
        "openTime": u64::MAX - 10,
        "closeTime": u64::MAX - 9,
        "closeReason": "normal",
        "iso": true,
        "isoPubkey": PUBKEY,
        "closeSlot": u64::MAX - 8,
        "sequence": u64::MAX - 7
    })
}

fn funding_row() -> Value {
    json!({
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "size": -1.25,
        "payment": 3.5,
        "fundingRate": 0.0001,
        "markPrice": 100_000.0,
        "iso": true,
        "isoPubkey": PUBKEY,
        "slot": u64::MAX - 6,
        "timestamp": u64::MAX - 5,
        "sequence": u64::MAX - 4
    })
}

fn order_row() -> Value {
    json!({
        "orderId": PUBKEY,
        "symbol": "BTC-USD",
        "side": "buy",
        "orderType": "limit",
        "tif": "gtc",
        "price": 100_000.0,
        "vwap": 99_999.0,
        "originalSize": 1.25,
        "executedSize": 1.25,
        "reduceOnly": false,
        "status": "filled",
        "trigger": {
            "isAbove": true,
            "px": 101_000.0,
            "lim": 100_500.0,
            "oco": PUBKEY,
            "pxHi": 102_000.0,
            "limHi": 101_500.0,
            "trb": 250,
            "stb": 25
        },
        "reason": "filled",
        "iso": true,
        "isoPubkey": PUBKEY,
        "slot": u64::MAX - 3,
        "timestamp": u64::MAX - 2,
        "sequence": u64::MAX - 1
    })
}

fn activity_row() -> Value {
    json!({
        "activityType": "transfer",
        "status": "settled",
        "from": PUBKEY,
        "to": PUBKEY,
        "symbol": "USDC",
        "amount": 25.0,
        "reason": "test",
        "iso": true,
        "isoPubkey": PUBKEY,
        "slot": u64::MAX - 12,
        "timestamp": u64::MAX - 11,
        "sequence": u64::MAX - 10
    })
}

fn risk_row() -> Value {
    json!({
        "owner": PUBKEY,
        "symbol": "BTC-USD",
        "isBuy": false,
        "amount": 0.5,
        "price": 80_000.0,
        "eventType": "risk_vault",
        "marginPrior": 10.0,
        "marginAfter": 2.0,
        "reason": "maintenance margin",
        "iso": true,
        "isoPubkey": PUBKEY,
        "slot": u64::MAX - 15,
        "timestamp": u64::MAX - 14,
        "sequence": u64::MAX - 13
    })
}

#[tokio::test]
async fn history_first_page_uses_exact_camel_case_query_and_preserves_u64() {
    let (url, request) = spawn_json_server("200 OK", page(fill_row())).await;
    let client = BulkHttpClient::with_url(&url, None).expect("create HTTP client");
    let response: HistoryPage<HistoryFill> = client
        .get_fills_page(
            PUBKEY,
            &HistoryQuery {
                limit: Some(500),
                cursor: None,
                start_slot: Some(9_007_199_254_740_993),
                end_slot: Some(u64::MAX),
            },
        )
        .await
        .expect("fetch fills page");

    assert_eq!(
        request
            .await
            .expect("captured request")
            .lines()
            .next()
            .expect("request line"),
        format!(
            "GET /accounts/{PUBKEY}/history/fills?limit=500&startSlot=9007199254740993&endSlot=18446744073709551615 HTTP/1.1"
        )
    );
    assert_eq!(response.data[0].slot, u64::MAX);
    assert_eq!(response.data[0].sequence, u64::MAX - 2);
    assert_eq!(response.page.as_of_slot, u64::MAX);
    assert_eq!(response.page.coverage, HistoryCoverageStatus::Complete);
}

#[tokio::test]
async fn history_continuation_sends_only_limit_and_cursor_and_does_not_auto_follow() {
    let (url, request) = spawn_json_server("200 OK", page(fill_row())).await;
    let client = BulkHttpClient::with_url(&url, None).expect("create HTTP client");
    let response = client
        .get_fills_page(
            PUBKEY,
            &HistoryQuery {
                limit: Some(17),
                cursor: Some("next_page".to_string()),
                start_slot: None,
                end_slot: None,
            },
        )
        .await
        .expect("fetch one continuation page");

    assert_eq!(
        request
            .await
            .expect("captured request")
            .lines()
            .next()
            .expect("request line"),
        format!("GET /accounts/{PUBKEY}/history/fills?limit=17&cursor=next_page HTTP/1.1")
    );
    assert!(response.page.has_more);
    assert_eq!(response.page.next_cursor.as_deref(), Some("next_page"));
}

#[tokio::test]
async fn history_methods_use_all_six_exact_paths_and_distinct_plain_rows() {
    let query = HistoryQuery::default();

    let (url, request) = spawn_json_server("200 OK", page(fill_row())).await;
    let fill: HistoryPage<HistoryFill> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_fills_page(PUBKEY, &query)
        .await
        .expect("fills");
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/fills HTTP/1.1")));
    assert!(fill.data[0].iso);

    let (url, request) = spawn_json_server("200 OK", page(position_row())).await;
    let position: HistoryPage<ClosedPosition> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_positions_page(PUBKEY, &query)
        .await
        .expect("positions");
    assert!(request.await.expect("request").starts_with(&format!(
        "GET /accounts/{PUBKEY}/history/positions HTTP/1.1"
    )));
    assert_eq!(position.data[0].close_slot, u64::MAX - 8);

    let (url, request) = spawn_json_server("200 OK", page(funding_row())).await;
    let funding: HistoryPage<FundingPayment> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_funding_page(PUBKEY, &query)
        .await
        .expect("funding");
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/funding HTTP/1.1")));
    assert_eq!(funding.data[0].sequence, u64::MAX - 4);

    let (url, request) = spawn_json_server("200 OK", page(order_row())).await;
    let order: HistoryPage<TerminalOrder> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_orders_page(PUBKEY, &query)
        .await
        .expect("orders");
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/orders HTTP/1.1")));
    assert_eq!(order.data[0].status, "filled");
    let trigger: &HistoryTrigger = order.data[0].trigger.as_ref().expect("typed trigger");
    assert_eq!(trigger.is_above, Some(true));
    assert_eq!(trigger.px, 101_000.0);
    assert_eq!(trigger.lim, Some(100_500.0));
    assert_eq!(trigger.oco.expect("OCO hash").to_string(), PUBKEY);
    assert_eq!(trigger.px_hi, Some(102_000.0));
    assert_eq!(trigger.lim_hi, Some(101_500.0));
    assert_eq!(trigger.trail_bps, Some(250));
    assert_eq!(trigger.step_bps, Some(25));
    assert_eq!(
        serde_json::to_value(&order.data[0]).expect("serialize typed order")["trigger"]["oco"],
        PUBKEY
    );

    let (url, request) = spawn_json_server("200 OK", page(activity_row())).await;
    let activity: HistoryPage<AccountActivity> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_activity_page(PUBKEY, &query)
        .await
        .expect("activity");
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/activity HTTP/1.1")));
    assert_eq!(activity.data[0].activity_type, "transfer");

    let (url, request) = spawn_json_server("200 OK", page(risk_row())).await;
    let risk: HistoryPage<RiskEvent> = BulkHttpClient::with_url(&url, None)
        .expect("client")
        .get_risk_page(PUBKEY, &query)
        .await
        .expect("risk");
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/risk HTTP/1.1")));
    assert_eq!(risk.data[0].event_type, RiskEventType::RiskVault);
}

#[test]
fn history_risk_rejects_undocumented_event_type() {
    let mut row = risk_row();
    row["eventType"] = json!("unknown");

    serde_json::from_value::<HistoryPage<RiskEvent>>(page(row))
        .expect_err("undocumented risk event type must not deserialize");
}

#[test]
fn history_order_rejects_malformed_oco_hash() {
    let mut row = order_row();
    row["trigger"]["oco"] = json!("not-a-base58-hash");

    let error = serde_json::from_value::<HistoryPage<TerminalOrder>>(page(row))
        .expect_err("malformed OCO must not deserialize");

    assert!(error.to_string().contains("base58"));
}

#[tokio::test]
async fn history_non_success_preserves_structured_error_status_and_body() {
    let (url, request) = spawn_json_server(
        "410 Gone",
        json!({
            "error": {
                "code": "CURSOR_EXPIRED",
                "message": "history changed"
            }
        }),
    )
    .await;
    let client = BulkHttpClient::with_url(&url, None).expect("create HTTP client");

    match client
        .get_risk_page(PUBKEY, &HistoryQuery::default())
        .await
        .expect_err("410 must remain an API error")
    {
        HistoryHttpError::Api { status, body } => {
            assert_eq!(status, StatusCode::GONE);
            assert_eq!(body.error.code, "CURSOR_EXPIRED");
            assert_eq!(body.error.message, "history changed");
        }
        HistoryHttpError::Transport(error) => panic!("unexpected transport error: {error}"),
    }
    assert!(request
        .await
        .expect("request")
        .starts_with(&format!("GET /accounts/{PUBKEY}/history/risk HTTP/1.1")));
}
