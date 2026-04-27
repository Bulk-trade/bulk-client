use eyre::bail;
use bulk_api::BulkHttpClient;
use crate::commands::CreateMultisigArgs;

pub async fn handle_create_multisig(
    api: &mut BulkHttpClient,
    args: CreateMultisigArgs
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

    // TODO: sign and submit create-multisig instruction

    Ok(())
}
