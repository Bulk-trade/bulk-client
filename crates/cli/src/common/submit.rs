use bulk_client::parts::make_nonce;
use bulk_client::transaction::canonical_message;
use bulk_client::transaction::Action;
use bulk_client::BulkHttpClient;

#[derive(Clone, Debug)]
pub struct SubmitOptions {
    pub preview: bool,
    pub auto_yes: bool,
}

pub async fn submit_actions(
    api: &mut BulkHttpClient,
    options: &SubmitOptions,
    actions: Vec<Action>,
) -> eyre::Result<()> {
    let nonce = make_nonce();
    let cfg = api.config();
    let signer = cfg
        .signer
        .as_ref()
        .ok_or_else(|| eyre::eyre!("signer required"))?;
    let account = signer.public_key();

    if options.preview {
        let preview = canonical_message(account, nonce, &actions)?;
        eprintln!("--- transaction preview ---");
        eprint!("{}", preview);
        if !options.auto_yes {
            use std::io::{self, Write};
            eprint!("Submit? [y/N]: ");
            io::stderr().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            if !matches!(buf.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                return Err(eyre::eyre!("transaction rejected by user"));
            }
        }
    }

    let results = api.place_tx(actions, None, Some(nonce)).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}
