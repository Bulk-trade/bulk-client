use eyre::bail;
use bulk_client::BulkHttpClient;
use bulk_client::msgs::multisig::{CreateMultisig, MultisigApprove, MultisigCancel, MultisigExecute, MultisigReject, UpdateMultisigPolicy};
use bulk_client::transaction::Action;
use crate::commands::{CreateMultisigArgs, MultisigProposalArgs, UpdateMultisigPolicyArgs};

// ---------------------------------------------------------------------------
// CreateMultisig
// ---------------------------------------------------------------------------

pub async fn handle_create_multisig(
    api: &mut BulkHttpClient,
    args: CreateMultisigArgs,
) -> eyre::Result<()> {
    if args.threshold == 0 {
        bail!("--threshold must be at least 1");
    }
    if args.threshold as usize > args.signers.len() {
        bail!(
            "--threshold {} exceeds signer count {}",
            args.threshold,
            args.signers.len()
        );
    }

    println!(
        "Creating {}-of-{} multisig  lock={}s  lifetime={}s",
        args.threshold,
        args.signers.len(),
        args.lock,
        args.lifetime,
    );
    for (i, pk) in args.signers.iter().enumerate() {
        println!("  signer[{i}] = {pk}");
    }

    let action = Action::CreateMultisig(CreateMultisig {
        signers: args.signers,
        threshold: args.threshold,
        time_lock_secs: args.lock,
        proposal_lifetime_secs: args.lifetime,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// UpdateMultisigPolicy
// ---------------------------------------------------------------------------

pub async fn handle_update_multisig_policy(
    api: &mut BulkHttpClient,
    args: UpdateMultisigPolicyArgs,
) -> eyre::Result<()> {
    if args.threshold == 0 {
        bail!("--threshold must be at least 1");
    }
    if args.threshold as usize > args.signers.len() {
        bail!(
            "--threshold {} exceeds signer count {}",
            args.threshold,
            args.signers.len()
        );
    }

    println!(
        "Updating multisig {} → {}-of-{}  lock={}s  lifetime={}s",
        args.multisig,
        args.threshold,
        args.signers.len(),
        args.lock,
        args.lifetime,
    );

    let action = Action::UpdateMultisigPolicy(UpdateMultisigPolicy {
        multisig: args.multisig,
        signers: args.signers,
        threshold: args.threshold,
        time_lock_secs: args.lock,
        proposal_lifetime_secs: args.lifetime,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Approve
// ---------------------------------------------------------------------------

pub async fn handle_multisig_approve(
    api: &mut BulkHttpClient,
    args: MultisigProposalArgs,
) -> eyre::Result<()> {
    println!("Approving proposal {} on multisig {}", args.proposal_id, args.multisig);

    let action = Action::MultisigApprove(MultisigApprove {
        multisig: args.multisig,
        proposal_id: args.proposal_id,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Reject
// ---------------------------------------------------------------------------

pub async fn handle_multisig_reject(
    api: &mut BulkHttpClient,
    args: MultisigProposalArgs,
) -> eyre::Result<()> {
    println!("Rejecting proposal {} on multisig {}", args.proposal_id, args.multisig);

    let action = Action::MultisigReject(MultisigReject {
        multisig: args.multisig,
        proposal_id: args.proposal_id,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cancel
// ---------------------------------------------------------------------------

pub async fn handle_multisig_cancel(
    api: &mut BulkHttpClient,
    args: MultisigProposalArgs,
) -> eyre::Result<()> {
    println!("Cancelling proposal {} on multisig {}", args.proposal_id, args.multisig);

    let action = Action::MultisigCancel(MultisigCancel {
        multisig: args.multisig,
        proposal_id: args.proposal_id,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}

// ---------------------------------------------------------------------------
// Execute
// ---------------------------------------------------------------------------

pub async fn handle_multisig_execute(
    api: &mut BulkHttpClient,
    args: MultisigProposalArgs,
) -> eyre::Result<()> {
    println!("Executing proposal {} on multisig {}", args.proposal_id, args.multisig);

    let action = Action::MultisigExecute(MultisigExecute {
        multisig: args.multisig,
        proposal_id: args.proposal_id,
        meta: Default::default(),
    });
    let results = api.place_tx(vec![action], None, None).await?;
    eprintln!("results: {:?}", results);
    Ok(())
}
