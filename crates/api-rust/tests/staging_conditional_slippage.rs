use bulk_client::api::parts::HttpConfig;
use bulk_client::api::BulkHttpClient;
use bulk_client::common::side::Side;
use bulk_client::msgs::conditional::{StopOrTP, Trailing};
use bulk_client::transaction::{Action, ActionMeta, SignatureDomain, TransactionSigner};
use eyre::{eyre, ContextCompat};
use solana_keypair::Keypair;
use std::env;
use std::sync::Arc;
use std::time::Duration;

const SYMBOL: &str = "BTC-USD";
const POSITION_SIZE: f64 = 0.001;
const STOP_SLIPPAGE_BPS: f64 = 37.0;
const TRAILING_SLIPPAGE_BPS: f64 = 41.0;

#[tokio::test]
#[ignore = "requires a live staging deployment with the faucet and market makers"]
async fn staging_conditional_slippage_flow() -> eyre::Result<()> {
    let keypair = Keypair::new();
    let private_key = bs58::encode(keypair.to_bytes()).into_string();
    let signer = TransactionSigner::from_private_key(&private_key)?;
    let account = signer.public_key();
    let base_url =
        env::var("BULK_STAGING_URL").unwrap_or_else(|_| "http://localhost:12000/api/v1".to_owned());
    let client = BulkHttpClient::new(&HttpConfig {
        base_url,
        signer: Some(signer),
        signature_domain: Some(SignatureDomain::Devnet),
        ..Default::default()
    })?;

    let faucet = client.request_faucet(None, None, None).await?;
    eyre::ensure!(!faucet.is_error(), "faucet request failed: {faucet:?}");

    let ticker = client.get_ticker(SYMBOL).await?;
    eyre::ensure!(
        ticker.mark_price.is_finite() && ticker.mark_price > 0.0,
        "invalid {SYMBOL} mark price: {}",
        ticker.mark_price
    );

    let market = client
        .place_market_order(
            SYMBOL,
            Side::Buy,
            POSITION_SIZE,
            false,
            None,
            None,
            Some(1_000.0),
        )
        .await?;
    eyre::ensure!(market.is_placement(), "position did not open: {market:?}");
    eprintln!(
        "open orders after market order: {:#?}",
        client.get_open_orders(&account.to_string()).await?
    );

    tokio::time::sleep(Duration::from_millis(250)).await;
    let position_size = client
        .get_account(account)
        .await?
        .positions
        .into_iter()
        .find(|position| position.symbol == SYMBOL && position.size > 0.0)
        .map(|position| position.size.abs())
        .ok_or_else(|| eyre!("long {SYMBOL} position was not present after the market fill"))?;

    let stop = Action::Stop(StopOrTP {
        symbol: Arc::from(SYMBOL),
        is_above: false,
        size: position_size,
        threshold: ticker.mark_price * 0.8,
        limit: None,
        iso: false,
        builder_code: None,
        slippage: Some(STOP_SLIPPAGE_BPS),
        meta: ActionMeta::default(),
    });
    let stop_response = client
        .place_tx(vec![stop], None, None)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("stop-loss request returned no status"))?;
    eyre::ensure!(
        !stop_response.is_error(),
        "stop-loss placement failed: {stop_response:?}"
    );
    let stop_order_id = stop_response
        .order_id
        .context("stop-loss placement returned no order id")?;
    eprintln!(
        "open orders after stop-loss: {:#?}",
        client.get_open_orders(&account.to_string()).await?
    );

    let cancel = client
        .cancel_order(SYMBOL, &stop_order_id, None, None)
        .await?;
    eyre::ensure!(
        !cancel.is_error(),
        "stop-loss cancellation failed: {cancel:?}"
    );
    eyre::ensure!(
        !client
            .get_open_orders(&account.to_string())
            .await?
            .iter()
            .any(|order| order.order_id == stop_order_id),
        "cancelled stop-loss remains open"
    );
    eprintln!(
        "open orders after stop-loss cancellation: {:#?}",
        client.get_open_orders(&account.to_string()).await?
    );

    let trailing = Action::Trailing(Trailing {
        symbol: Arc::from(SYMBOL),
        is_buy: true,
        size: position_size,
        trail_bps: 200,
        step_bps: 50,
        limit: None,
        iso: false,
        builder_code: None,
        slippage: Some(TRAILING_SLIPPAGE_BPS),
        meta: ActionMeta::default(),
    });
    let trailing_response = client
        .place_tx(vec![trailing], None, None)
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("trailing-order request returned no status"))?;
    eyre::ensure!(
        !trailing_response.is_error(),
        "trailing-order placement failed: {trailing_response:?}"
    );
    let trailing_order_id = trailing_response
        .order_id
        .context("trailing-order placement returned no order id")?;
    eyre::ensure!(
        client
            .get_open_orders(&account.to_string())
            .await?
            .iter()
            .any(|order| order.order_id == trailing_order_id),
        "trailing order was accepted but is not open"
    );
    eprintln!(
        "open orders after trailing order: {:#?}",
        client.get_open_orders(&account.to_string()).await?
    );

    eprintln!(
        "staging flow passed for {account}: faucet, long {SYMBOL}, stop {stop_order_id} at {STOP_SLIPPAGE_BPS}bps, cancel, trailing {trailing_order_id} at {TRAILING_SLIPPAGE_BPS}bps"
    );
    Ok(())
}
