mod brain;
mod obsidian;
mod security;

use brain::{
    ActiveCopyPosition, AlphaDiscoveryBrain, CopySignalVerdict, ExecutionMode, PositionAction,
    ScalingTier,
};
use obsidian::{obsidian_vault_worker, ObsidianVaultEvent};
use security::{
    BondingCurveState, JitoAtomicBundleSpec, LiveChartMetrics, SecurityDecision,
    ZeroCopySplMintView, PUMPFUN_GRADUATION_SOL_LAMPORTS,
};
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let vault_root = PathBuf::from("./obsidian_vault");
    let checkpoint_path = vault_root.join("brain_checkpoint.json");

    let mut brain = AlphaDiscoveryBrain::load_or_init(&checkpoint_path, ExecutionMode::ShadowTraining);
    println!(
        "[SYSTEM ONLINE] Mode: {:?} | Profiles: {} | Dynamic Dev Cap: {} BPS | Bankroll: {:.2} SOL",
        brain.mode,
        brain.profiles.len(),
        brain.security_regime.dynamic_max_dev_share_bps,
        brain.simulated_bankroll_sol
    );

    let (tx, rx) = mpsc::channel::<ObsidianVaultEvent>(4096);
    let worker_handle = tokio::spawn(obsidian_vault_worker(rx, vault_root.clone()));

    // 1. ORIGINAL + ADAPTIVE TRAINING: Observe `AlphaWhale_01` across 3 winning launches
    let insider = "AlphaWhale_01_999999999999999999999999999999";
    let training_observations = [
        (1_000_000_000, 4_200_000_000, false),
        (1_500_000_000, 10_500_000_000, false),
        (2_000_000_000, 9_000_000_000, false),
    ];

    for (entry, exit, dumped_fast) in training_observations {
        let updated_profile = brain
            .record_wallet_trade(insider, entry, exit, dumped_fast)
            .clone();
        let _ = tx.try_send(ObsidianVaultEvent::SyncWalletNode {
            profile: updated_profile,
        });
    }

    // 2. ORIGINAL HONEYPOT GATE: Catch & Quarantine Freeze-Authority Honeypot (`ScamFreezeMint`)
    let mut bad_mint_bytes = [0u8; 82];
    bad_mint_bytes[46] = 1; // Active freeze_authority
    let gate_clock = Instant::now();
    if let Ok(view) = ZeroCopySplMintView::try_from_slice(&bad_mint_bytes) {
        if let Err(verdict) = view.verify_security_invariants() {
            let _ = tx.try_send(ObsidianVaultEvent::RecordQuarantinedMint {
                mint: "ScamFreezeMint_111111111111111111111111pump".to_owned(),
                creator: "RugDeployer_Wallet_01".to_owned(),
                verdict,
                latency_ns: gate_clock.elapsed().as_nanos(),
            });
        }
    }

    // Setup synthetic Bonding Curve & Clean Mint bytes
    let clean_mint_bytes = [0u8; 82];
    let mut curve_bytes = [0u8; 49];
    curve_bytes[8..16].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    curve_bytes[16..24].copy_from_slice(&32_000_000_000u64.to_le_bytes());
    curve_bytes[24..32].copy_from_slice(&790_000_000_000_000u64.to_le_bytes());
    curve_bytes[40..48].copy_from_slice(&1_000_000_000_000_000u64.to_le_bytes());
    let base_curve = BondingCurveState::try_from_bytes(&curve_bytes).unwrap();

    // 3. NEW ADAPTIVE PROBATION GATE: Multi-Wallet Cabal Bundle detected -> `HoldAndObserve`
    let cabal_decision = brain.security_regime.evaluate_play(
        &clean_mint_bytes,
        &base_curve,
        50_000_000_000_000,
        4, // 4 synchronized cabal wallets in the same slot
        0,
    );
    if let SecurityDecision::HoldAndObserve { wait_slots, reason } = cabal_decision {
        let _ = tx.try_send(ObsidianVaultEvent::RecordProbationaryHold {
            mint: "SuspiciousCabalToken_1111111111111111111pump".to_owned(),
            insider_wallet: "CabalCluster_Group_A".to_owned(),
            wait_slots,
            reason,
        });
    }

    // 4. BONDED SLOT-0 COPY-TRADE + JITO ANTI-SANDWICH BUNDLE + 2.5x->1000x MOONBAG LIFECYCLE
    let target_mint = "OutlierRunner_1111111111111111111111111pump";
    let snipe_clock = Instant::now();

    if brain.is_bookmarked(insider) {
        if let CopySignalVerdict::ApprovedCopy { .. } =
            brain.audit_live_insider_entry(insider, 1_500_000_000, 0)
        {
            if let SecurityDecision::ClearForImmediateEntry { confidence_bps } =
                brain.security_regime.evaluate_play(
                    &clean_mint_bytes,
                    &base_curve,
                    40_000_000_000_000,
                    0,
                    3, // 3 verified external sells
                )
            {
                let entry_chart = LiveChartMetrics::from_curve(&base_curve, 200.0);
                let entry_sol = 1.0;
                let entry_lamports = 1_000_000_000;
                brain.simulated_bankroll_sol -= entry_sol;

                let jito_bundle = JitoAtomicBundleSpec::build_protected_snipe(
                    target_mint.to_owned(),
                    &base_curve,
                    entry_lamports,
                    250,       // 2.5% max slippage
                    1_200_000, // Competing priority fee
                );

                let mut pos = ActiveCopyPosition {
                    mint_pubkey: target_mint.to_owned(),
                    copied_insider_wallet: insider.to_owned(),
                    entry_sol_amount: entry_sol,
                    realized_sol_returned: 0.0,
                    entry_mcap_usd: entry_chart.market_cap_usd,
                    peak_mcap_usd: entry_chart.market_cap_usd,
                    remaining_tokens: jito_bundle.expected_tokens_out,
                    tier: ScalingTier::InitialRiskActive,
                };

                let _ = tx.try_send(ObsidianVaultEvent::RecordCopyEntry {
                    mode: brain.mode,
                    mint: target_mint.to_owned(),
                    insider_wallet: insider.to_owned(),
                    entry_mcap_usd: entry_chart.market_cap_usd,
                    confidence_bps,
                    jito_bundle,
                    latency_ns: snipe_clock.elapsed().as_nanos(),
                });

                // Tuple: (virtual_sol_reserves, insider_sold, external_sells_succeeded, cabal_dumping)
                let reserve_ticks: &[(u64, bool, u32, u32)] = &[
                    (80_000_000_000, false, 12, 0),      // 2.5x ($16k MCap) -> Principal Recovered!
                    (320_000_000_000, false, 45, 0),     // 10x ($64k MCap) -> 10x Banked!
                    (3_200_000_000_000, false, 210, 0),  // 100x ($640k MCap) -> 100x Banked!
                    (32_000_000_000_000, false, 890, 0), // 1000x ($6.4M MCap) -> 1000x Outlier Locked!
                    (25_000_000_000_000, true, 950, 0),  // Insider exits -> Full Moonbag Closed!
                ];

                for &(v_sol, insider_sold, ext_sells, cabal_dumps) in reserve_ticks {
                    let mut tick_curve = base_curve;
                    tick_curve.virtual_sol_reserves = v_sol;
                    tick_curve.real_sol_reserves = v_sol.min(PUMPFUN_GRADUATION_SOL_LAMPORTS);
                    let live_chart = LiveChartMetrics::from_curve(&tick_curve, 200.0);

                    if let PositionAction::ScaleOut {
                        sol_proceeds,
                        current_multiple,
                        reason,
                        ..
                    } = pos.evaluate_market_tick_full(&live_chart, insider_sold, ext_sells, cabal_dumps)
                    {
                        brain.simulated_bankroll_sol += sol_proceeds;
                        let _ = tx.try_send(ObsidianVaultEvent::RecordScaleOut {
                            mode: brain.mode,
                            mint: target_mint.to_owned(),
                            insider_wallet: insider.to_owned(),
                            multiple: current_multiple,
                            mcap_usd: live_chart.market_cap_usd,
                            sol_proceeds,
                            simulated_bankroll_sol: brain.simulated_bankroll_sol,
                            reason,
                        });
                    }
                }
            }
        }
    }

    brain.save_checkpoint(&checkpoint_path);
    drop(tx);
    let _ = worker_handle.await;
    println!(
        "[CHECKPOINT SAVED] Final Virtual Bankroll: {:.2} SOL. Original + Adaptive code 100% bonded!",
        brain.simulated_bankroll_sol
    );
}