use bulk_client::msgs::MultisigPropose;
use bulk_client::msgs::Response;
use bulk_client::api::parts::make_nonce;

use bulk_client::transaction::canonical_message;
use bulk_client::transaction::{Action, ActionMeta};
use bulk_client::BulkHttpClient;
use solana_pubkey::Pubkey;
use std::fmt::Write as _;
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

    let action_debug = proposed_action_debug(&actions);
    let results = api.place_tx(actions, None, Some(nonce)).await?;
    eprint!("{}", format_results(&results, &action_debug));
    Ok(())
}

fn format_results(results: &[Response], action_debug: &[String]) -> String {
    if let Some(created) = results
        .iter()
        .find(|response| response.status == "proposalCreated")
    {
        return format_proposal_created(created, action_debug);
    }

    let approval = results
        .iter()
        .find(|response| response.status == "proposalApproved");
    let proposal_outcome = results.iter().rev().find(|response| {
        matches!(
            response.status.as_str(),
            "proposalFailed"
                | "proposalExecuted"
                | "proposalReadyForExecution"
                | "proposalRejected"
        )
    });

    if approval.is_some() || proposal_outcome.is_some() {
        return format_approval_result(approval, proposal_outcome);
    }

    let mut output = String::from("\nStatus\n");
    output.push_str("────────────────────────────────────────\n");
    if results.is_empty() {
        output.push_str("  No response statuses returned\n");
        return output;
    }
    for response in results {
        let _ = writeln!(output, "  {}", humanize_status(&response.status));
        if let Some(message) = &response.message {
            let _ = writeln!(output, "    Message: {message}");
        }
    }
    output
}

fn format_proposal_created(created: &Response, action_debug: &[String]) -> String {
    let mut output = String::from("\nProposal Created\n");
    output.push_str("────────────────────────────────────────\n");
    write_json_field(&mut output, "Proposal", &created.raw, "proposalId");
    write_json_field(&mut output, "Required signers", &created.raw, "threshold");
    output.push_str("\nActions\n");
    output.push_str("────────────────────────────────────────\n");
    if action_debug.is_empty() {
        output.push_str("  No action details available\n");
    } else {
        for (index, action) in action_debug.iter().enumerate() {
            let _ = writeln!(output, "  [{index}] {action}");
        }
    }
    output
}

fn proposed_action_debug(actions: &[Action]) -> Vec<String> {
    actions
        .iter()
        .flat_map(|action| match action {
            Action::MultisigPropose(proposal) => proposal
                .actions
                .iter()
                .map(|nested| format!("{nested:?}"))
                .collect(),
            ordinary => vec![format!("{ordinary:?}")],
        })
        .collect()
}

fn format_approval_result(approval: Option<&Response>, outcome: Option<&Response>) -> String {
    let details = approval.or(outcome).expect("approval result exists");
    let mut output = String::from("\nApprovals\n");
    output.push_str("────────────────────────────────────────\n");
    write_json_field(&mut output, "Proposal", &details.raw, "proposalId");
    write_json_field(&mut output, "Multisig", &details.raw, "multisig");
    let approvals = details
        .raw
        .get("approvals")
        .and_then(|value| value.as_u64());
    let threshold = details
        .raw
        .get("threshold")
        .and_then(|value| value.as_u64());
    if let (Some(approvals), Some(threshold)) = (approvals, threshold) {
        let _ = writeln!(output, "  Progress: {approvals} / {threshold} approvals");
    }
    write_json_field(&mut output, "Rejections", &details.raw, "rejections");
    write_json_field(&mut output, "Approved by", &details.raw, "signer");

    output.push_str("\nOutcome\n");
    output.push_str("────────────────────────────────────────\n");
    let final_response = outcome.or(approval).expect("approval result exists");
    let _ = writeln!(
        output,
        "  Status: {}",
        humanize_status(&final_response.status)
    );
    if let Some(message) = &final_response.message {
        let _ = writeln!(output, "  Error: {message}");
    } else if final_response.status == "proposalReadyForExecution" {
        write_json_field(
            &mut output,
            "Execute after",
            &final_response.raw,
            "executeAfter",
        );
    }
    output
}

fn write_json_field(output: &mut String, label: &str, body: &serde_json::Value, field: &str) {
    let Some(value) = body.get(field) else {
        return;
    };
    if value.is_null() {
        return;
    }
    let display = value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    let _ = writeln!(output, "  {label}: {display}");
}

fn humanize_status(status: &str) -> String {
    let mut output = String::with_capacity(status.len() + 4);
    for (index, character) in status.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            output.push(' ');
        }
        if index == 0 {
            output.extend(character.to_uppercase());
        } else {
            output.push(character.to_ascii_lowercase());
        }
    }
    output
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
                    proposal_lifetime_secs: None,
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
        assert_eq!(first.proposal_lifetime_secs, None);
        assert_eq!(second.proposal_lifetime_secs, None);
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

    #[test]
    fn formats_threshold_approval_and_failed_execution() {
        let common = serde_json::json!({
            "multisig": ADMIN_MULTISIG,
            "proposalId": 17,
            "approvals": 2,
            "rejections": 0,
            "threshold": 2,
            "signer": "Signer111"
        });
        let results = vec![
            response("proposalApproved", None, common.clone()),
            response("proposalReadyForExecution", None, common.clone()),
            response(
                "proposalFailed",
                Some("minimum tier exceeds active tier count"),
                common,
            ),
        ];

        let output = format_results(&results, &[]);

        assert!(output.contains("Approvals"));
        assert!(output.contains("Progress: 2 / 2 approvals"));
        assert!(output.contains("Approved by: Signer111"));
        assert!(output.contains("Outcome"));
        assert!(output.contains("Status: Proposal failed"));
        assert!(output.contains("Error: minimum tier exceeds active tier count"));
    }

    #[test]
    fn formats_approval_awaiting_threshold() {
        let result = response(
            "proposalApproved",
            None,
            serde_json::json!({
                "proposalId": 18,
                "approvals": 1,
                "rejections": 0,
                "threshold": 2
            }),
        );

        let output = format_results(&[result], &[]);

        assert!(output.contains("Progress: 1 / 2 approvals"));
        assert!(output.contains("Status: Proposal approved"));
    }

    #[test]
    fn formats_created_proposal_with_threshold_and_nested_action_debug() {
        let created = response(
            "proposalCreated",
            None,
            serde_json::json!({
                "proposalId": 23,
                "threshold": 2
            }),
        );
        let actions =
            vec!["PricingAdmin(PricingAdmin { instrument: \"BTC\", source: Bulk })".to_string()];

        let output = format_results(&[created], &actions);

        assert!(output.contains("Proposal Created"));
        assert!(output.contains("Proposal: 23"));
        assert!(output.contains("Required signers: 2"));
        assert!(output.contains("[0] PricingAdmin"));
        assert!(!output.contains("MultisigPropose"));
    }

    fn response(status: &str, message: Option<&str>, raw: serde_json::Value) -> Response {
        Response {
            order_id: None,
            status: status.to_owned(),
            message: message.map(str::to_owned),
            raw,
        }
    }
}
