use bulk_client::msgs::MultisigPropose;
use bulk_client::parts::make_nonce;
use bulk_client::transaction::canonical_message;
use bulk_client::transaction::{Action, ActionMeta};
use bulk_client::BulkHttpClient;
use solana_pubkey::Pubkey;
use std::str::FromStr;

const ADMIN_MULTISIG: &str = "ADM1N11111111111111111111111111111111111113D";

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
    let actions = wrap_admin_actions(actions);
    let nonce = make_nonce();
    let cfg = api.config();
    let signer = cfg
        .signer
        .as_ref()
        .ok_or_else(|| eyre::eyre!("signer required"))?;
    let account = signer.public_key();

    if options.preview {
        let preview = canonical_message(
            cfg.signature_domain
                .ok_or_else(|| eyre::eyre!("signature domain required"))?,
            account,
            nonce,
            &actions,
        )?;
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

/// Wraps protected CLI actions in proposals to the protocol administrative multisig.
///
/// - Wraps every protected action in its own single-action proposal.
/// - Preserves the original ordering of administrative and ordinary actions.
/// - Leaves existing multisig proposals and non-admin actions unchanged.
fn wrap_admin_actions(actions: Vec<Action>) -> Vec<Action> {
    let admin_multisig = Pubkey::from_str(ADMIN_MULTISIG).expect("valid admin multisig pubkey");
    actions
        .into_iter()
        .map(|action| {
            if action.is_admin_multisig_action() {
                Action::MultisigPropose(MultisigPropose {
                    multisig: admin_multisig,
                    actions: vec![action],
                    meta: ActionMeta::default(),
                })
            } else {
                action
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bulk_client::msgs::{AddMarket, CancelAll};

    #[test]
    fn wraps_each_admin_action_once() {
        let wrapped = wrap_admin_actions(vec![
            Action::AddMarket(AddMarket {
                symbol: "BTC-USD".into(),
                meta: ActionMeta::default(),
            }),
            Action::AddMarket(AddMarket {
                symbol: "ETH-USD".into(),
                meta: ActionMeta::default(),
            }),
        ]);

        let [Action::MultisigPropose(first), Action::MultisigPropose(second)] = wrapped.as_slice()
        else {
            panic!("each admin action must be wrapped separately");
        };
        let expected_multisig = Pubkey::from_str(ADMIN_MULTISIG).unwrap();
        assert_eq!(first.multisig, expected_multisig);
        assert_eq!(second.multisig, expected_multisig);
        assert_eq!(first.actions.len(), 1);
        assert_eq!(second.actions.len(), 1);
        assert!(matches!(first.actions[0], Action::AddMarket(_)));
        assert!(matches!(second.actions[0], Action::AddMarket(_)));

        let wrapped_again = wrap_admin_actions(wrapped);
        assert!(matches!(
            wrapped_again.as_slice(),
            [Action::MultisigPropose(_), Action::MultisigPropose(_)]
        ));
    }

    #[test]
    fn leaves_ordinary_actions_unwrapped() {
        let wrapped = wrap_admin_actions(vec![Action::CancelAll(CancelAll {
            symbols: Vec::new(),
            meta: ActionMeta::default(),
        })]);

        assert!(matches!(wrapped.as_slice(), [Action::CancelAll(_)]));
    }
}
