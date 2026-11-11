use crate::brain::{ExecutionMode, TrackedWalletProfile, WalletClassification};
use crate::security::{JitoAtomicBundleSpec, SolanaThreatVerdict};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::{self, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ObsidianVaultEvent {
    SyncWalletNode {
        profile: TrackedWalletProfile,
    },
    RecordProbationaryHold {
        mint: String,
        insider_wallet: String,
        wait_slots: u64,
        reason: &'static str,
    },
    RecordCopyEntry {
        mode: ExecutionMode,
        mint: String,
        insider_wallet: String,
        entry_mcap_usd: f64,
        confidence_bps: u16,
        jito_bundle: JitoAtomicBundleSpec,
        latency_ns: u128,
    },
    RecordScaleOut {
        mode: ExecutionMode,
        mint: String,
        insider_wallet: String,
        multiple: f64,
        mcap_usd: f64,
        sol_proceeds: f64,
        simulated_bankroll_sol: f64,
        reason: &'static str,
    },
    RecordQuarantinedMint {
        mint: String,
        creator: String,
        verdict: SolanaThreatVerdict,
        latency_ns: u128,
    },
}

pub async fn obsidian_vault_worker(
    mut rx: mpsc::Receiver<ObsidianVaultEvent>,
    vault_root: PathBuf,
) {
    let wallets_dir = vault_root.join("Wallets");
    let tokens_dir = vault_root.join("Tokens");
    let ledger_dir = vault_root.join("Ledger");

    let _ = fs::create_dir_all(&wallets_dir).await;
    let _ = fs::create_dir_all(&tokens_dir).await;
    let _ = fs::create_dir_all(&ledger_dir).await;

    let dashboard_path = vault_root.join("00_Command_Center.md");
    if !dashboard_path.exists() {
        let dashboard_md = r#"---
tags:
  - command-center
  - solana-bot
---
# Solana Slot-0 Autonomous Training & Execution Center

## 1. Auto-Bookmarked Insider Wallets (Live Dataview Table)
```dataview
TABLE status AS "Classification", ewma_multiple AS "EWMA Multiple", win_rate AS "Win Rate (%)", peak_multiple AS "Peak Hit (x)", cabal_strikes AS "Cabal Strikes", total_trades AS "Trades"
FROM "Wallets"
WHERE status = "BookmarkedInsider" OR status = "ProbationarySuspect"
SORT ewma_multiple DESC
2. Active & Completed Moonbag Positions (10x - 1000x Tracker)
Code snippet
TABLE copied_insider AS "Insider", highest_multiple AS "Peak Multiple (x)", total_sol_returned AS "SOL Banked", latest_status AS "Stage"
FROM "Tokens"
WHERE type = "copy-trade"
SORT highest_multiple DESC
3. Quarantined Rugs, Honeypots & Probationary Holds
Code snippet
TABLE verdict AS "Threat / Hold Verdict", creator AS "Deployer / Actor", detection_ns AS "Gate Latency (ns)"
FROM "Tokens"
WHERE type = "quarantined-rug" OR type = "probationary-hold"
"#;
let _ = fs::write(&dashboard_path, dashboard_md).await;
}

let ledger_path = ledger_dir.join("Shadow_Training_Log.md");

while let Some(event) = rx.recv().await {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    match event {
        ObsidianVaultEvent::SyncWalletNode { profile } => {
            let win_rate = if profile.total_trades > 0 {
                (profile.winning_trades as f64 / profile.total_trades as f64) * 100.0
            } else {
                0.0
            };
            let tag = match profile.status {
                WalletClassification::BookmarkedInsider => "bookmarked-insider",
                WalletClassification::ProbationarySuspect => "probationary-suspect",
                WalletClassification::BlacklistedDumper => "blacklisted-dumper",
                WalletClassification::Observing => "observing-wallet",
            };

            let wallet_note = format!(
                "---\ntype: wallet-profile\ntags:\n  - {}\nstatus: \"{:?}\"\newma_multiple: {:.2}\nwin_rate: {:.1}\npeak_multiple: {:.1}\ncabal_strikes: {}\ntotal_trades: {}\nupdated: {}\n---\n# Insider Profile: `{}`\n- **Classification**: `{:?}`\n- **EWMA Return Multiple**: `{:.2}x`\n- **Win Rate**: `{:.1}%` (`{}/{}` trades)\n- **Peak Multiplier Observed**: `{:.1}x`\n- **Average Bet Size**: `{} Lamports`\n- **Insider Group (Cabal) Strikes**: `{}`\n",
                tag,
                profile.status,
                profile.ewma_multiple,
                win_rate,
                profile.peak_multiple_seen,
                profile.cabal_cluster_strikes,
                profile.total_trades,
                ts,
                profile.wallet_pubkey,
                profile.status,
                profile.ewma_multiple,
                win_rate,
                profile.winning_trades,
                profile.total_trades,
                profile.peak_multiple_seen,
                profile.avg_entry_lamports,
                profile.cabal_cluster_strikes
            );
            let _ = fs::write(wallets_dir.join(format!("{}.md", profile.wallet_pubkey)), wallet_note).await;
        }

        ObsidianVaultEvent::RecordProbationaryHold {
            mint,
            insider_wallet,
            wait_slots,
            reason,
        } => {
            let hold_note = format!(
                "---\ntype: probationary-hold\ntags:\n  - probationary-hold\ncreator: \"[[{}]]\"\nverdict: \"Hold {} Slots: {}\"\ndetection_ns: 0\n---\n# Probationary Hold (Did Not React Blindly): `{}`\n- **Actor**: [[{}]]\n- **Wait Duration**: `{} Slots`\n- **Suspicion Reason**: {}\n",
                insider_wallet, wait_slots, reason, mint, insider_wallet, wait_slots, reason
            );
            let _ = fs::write(tokens_dir.join(format!("{}.md", mint)), hold_note).await;
        }

        ObsidianVaultEvent::RecordCopyEntry {
            mode,
            mint,
            insider_wallet,
            entry_mcap_usd,
            confidence_bps,
            jito_bundle,
            latency_ns,
        } => {
            let token_note = format!(
                "---\ntype: copy-trade\ntags:\n  - moonbag-position\nmode: \"{:?}\"\ncopied_insider: \"[[{}]]\"\nentry_mcap_usd: {:.2}\nregime_confidence_bps: {}\nhighest_multiple: 1.0\ntotal_sol_returned: 0.0\nlatest_status: \"Entered Slot-0\"\n---\n# Token Position: `{}`\n- **Copied Insider**: [[{}]]\n- **Entry Market Cap**: `${:.2}`\n- **Regime Confidence**: `{:.2}%`\n- **Jito Tip**: `{} Lamports` | **Anti-Sandwich Guard**: `{}`\n- **Reaction Latency**: `{} ns`\n\n## Scaling Lifecycle\n",
                mode,
                insider_wallet,
                entry_mcap_usd,
                confidence_bps,
                mint,
                insider_wallet,
                entry_mcap_usd,
                (confidence_bps as f64) / 100.0,
                jito_bundle.jito_tip_lamports,
                jito_bundle.read_only_guard_account,
                latency_ns
            );
            let _ = fs::write(tokens_dir.join(format!("{}.md", mint)), token_note).await;
        }

        ObsidianVaultEvent::RecordScaleOut {
            mode,
            mint,
            insider_wallet,
            multiple,
            mcap_usd,
            sol_proceeds,
            simulated_bankroll_sol,
            reason,
        } => {
            let token_note = format!(
                "---\ntype: copy-trade\ntags:\n  - moonbag-position\nmode: \"{:?}\"\ncopied_insider: \"[[{}]]\"\nhighest_multiple: {:.1}\ntotal_sol_returned: {:.4}\nlatest_status: \"{}\"\n---\n# Token Position: `{}`\n- **Copied Insider**: [[{}]]\n- **Latest Milestone**: `{:.1}x` (`${:.2}` Market Cap)\n- **Trigger**: {}\n- **SOL Returned on Step**: `{:.4} SOL`\n- **Updated Virtual Bankroll**: `{:.4} SOL`\n",
                mode, insider_wallet, multiple, sol_proceeds, reason, mint, insider_wallet, multiple, mcap_usd, reason, sol_proceeds, simulated_bankroll_sol
            );
            let _ = fs::write(tokens_dir.join(format!("{}.md", mint)), token_note).await;

            let log_line = format!(
                "- `[{}]` **[{:?}]** [[{}]] hit **{:.1}x** (`${:.0}` MCap) via [[{}]] -> {} (`+{:.2} SOL` | Bankroll: `{:.2} SOL`)\n",
                ts, mode, mint, multiple, mcap_usd, insider_wallet, reason, sol_proceeds, simulated_bankroll_sol
            );
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&ledger_path).await {
                let _ = file.write_all(log_line.as_bytes()).await;
            }
        }

        ObsidianVaultEvent::RecordQuarantinedMint {
            mint,
            creator,
            verdict,
            latency_ns,
        } => {
            let rug_note = format!(
                "---\ntype: quarantined-rug\ntags:\n  - quarantined-rug\ncreator: \"[[{}]]\"\nverdict: \"{:?}\"\ndetection_ns: {}\n---\n# Quarantined Mint: `{}`\n- **Deployer**: [[{}]]\n- **Blocked Reason**: `{:?}`\n- **Gate Speed**: `{} ns`\n",
                creator, verdict, latency_ns, mint, creator, verdict, latency_ns
            );
            let _ = fs::write(tokens_dir.join(format!("{}.md", mint)), rug_note).await;
        }
    }
}
}
