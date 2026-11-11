use crate::security::{AdaptiveSecurityEngine, LiveChartMetrics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    ShadowTraining,
    LiveJitoMainnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletClassification {
    Observing,
    BookmarkedInsider,
    ProbationarySuspect,
    BlacklistedDumper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopySignalVerdict {
    ApprovedCopy { recommended_scale_bps: u64 },
    SkipSuspiciousPlay { reason: &'static str },
    RevokeBookmarkAndBlacklist { reason: &'static str },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedWalletProfile {
    pub wallet_pubkey: String,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub ewma_multiple: f64,
    pub peak_multiple_seen: f64,
    pub avg_entry_lamports: u64,
    pub cabal_cluster_strikes: u32,
    pub rug_dump_count: u32,
    pub status: WalletClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlphaDiscoveryBrain {
    pub mode: ExecutionMode,
    pub simulated_bankroll_sol: f64,
    pub security_regime: AdaptiveSecurityEngine,
    pub profiles: HashMap<String, TrackedWalletProfile>,
}

impl AlphaDiscoveryBrain {
    pub fn load_or_init(checkpoint_path: &Path, mode: ExecutionMode) -> Self {
        if let Ok(raw) = fs::read_to_string(checkpoint_path) {
            if let Ok(mut loaded) = serde_json::from_str::<Self>(&raw) {
                loaded.mode = mode;
                return loaded;
            }
        }
        Self {
            mode,
            simulated_bankroll_sol: 10.0,
            security_regime: AdaptiveSecurityEngine::new(),
            profiles: HashMap::with_capacity(10_000),
        }
    }

    pub fn save_checkpoint(&self, checkpoint_path: &Path) {
        if let Ok(serialized) = serde_json::to_string_pretty(self) {
            let _ = fs::write(checkpoint_path, serialized);
        }
    }

    /// Original 4-argument training method preserved for backward compatibility
    pub fn record_wallet_trade(
        &mut self,
        wallet: &str,
        entry_lamports: u64,
        exit_lamports: u64,
        dumped_under_30_seconds: bool,
    ) -> &TrackedWalletProfile {
        self.record_wallet_trade_full(wallet, entry_lamports, exit_lamports, dumped_under_30_seconds, false)
    }

    /// Extended training method that also tracks synchronized Insider Group (Cabal) clusters
    /// and updates the global AdaptiveSecurityEngine.
    pub fn record_wallet_trade_full(
        &mut self,
        wallet: &str,
        entry_lamports: u64,
        exit_lamports: u64,
        dumped_under_30_seconds: bool,
        traded_with_cabal_cluster: bool,
    ) -> &TrackedWalletProfile {
        let multiple = if entry_lamports > 0 {
            (exit_lamports as f64) / (entry_lamports as f64)
        } else {
            1.0
        };

        self.security_regime
            .record_market_regime_outcome(dumped_under_30_seconds || multiple < 0.30);

        let profile = self.profiles.entry(wallet.to_owned()).or_insert(TrackedWalletProfile {
            wallet_pubkey: wallet.to_owned(),
            total_trades: 0,
            winning_trades: 0,
            ewma_multiple: 1.0,
            peak_multiple_seen: 1.0,
            avg_entry_lamports: entry_lamports,
            cabal_cluster_strikes: 0,
            rug_dump_count: 0,
            status: WalletClassification::Observing,
        });

        profile.total_trades += 1;
        if multiple > 1.15 && !dumped_under_30_seconds {
            profile.winning_trades += 1;
        }
        if multiple > profile.peak_multiple_seen {
            profile.peak_multiple_seen = multiple;
        }
        if dumped_under_30_seconds {
            profile.rug_dump_count += 1;
        }
        if traded_with_cabal_cluster {
            profile.cabal_cluster_strikes += 1;
        }

        profile.avg_entry_lamports =
            ((profile.avg_entry_lamports as u128 * 7 + entry_lamports as u128 * 3) / 10) as u64;

        profile.ewma_multiple = (profile.ewma_multiple * 0.70) + (multiple * 0.30);
        let win_rate = (profile.winning_trades as f64) / (profile.total_trades as f64);

        if profile.rug_dump_count >= 2
            || profile.cabal_cluster_strikes >= 3
            || (profile.total_trades >= 3 && profile.ewma_multiple < 0.85)
        {
            profile.status = WalletClassification::BlacklistedDumper;
        } else if traded_with_cabal_cluster || dumped_under_30_seconds {
            profile.status = WalletClassification::ProbationarySuspect;
        } else if profile.total_trades >= 3 && win_rate >= 0.60 && profile.ewma_multiple >= 3.0 {
            profile.status = WalletClassification::BookmarkedInsider;
        }

        profile
    }

    /// Original fast bookmark check preserved
    #[inline(always)]
    pub fn is_bookmarked(&self, wallet: &str) -> bool {
        self.profiles
            .get(wallet)
            .map(|p| p.status == WalletClassification::BookmarkedInsider)
            .unwrap_or(false)
    }

    /// Bonded Pre-Trade Surveillance: catches patient scammers executing a 4x bet-size bait spike
    /// or coordinating with an insider cabal group.
    #[inline]
    pub fn audit_live_insider_entry(
        &mut self,
        wallet: &str,
        live_entry_lamports: u64,
        co_buying_cabal_wallets: u32,
    ) -> CopySignalVerdict {
        let Some(profile) = self.profiles.get_mut(wallet) else {
            return CopySignalVerdict::SkipSuspiciousPlay {
                reason: "Unverified Wallet",
            };
        };

        if profile.status != WalletClassification::BookmarkedInsider {
            return CopySignalVerdict::SkipSuspiciousPlay {
                reason: "Wallet Not in BookmarkedInsider State",
            };
        }

        if profile.avg_entry_lamports > 0 && live_entry_lamports > profile.avg_entry_lamports * 4 {
            profile.status = WalletClassification::ProbationarySuspect;
            return CopySignalVerdict::SkipSuspiciousPlay {
                reason: "Abnormal 4x+ Bet Size Spike (Copy-Trader Bait Trap)",
            };
        }

        if co_buying_cabal_wallets >= 3 {
            profile.cabal_cluster_strikes += 1;
            if profile.cabal_cluster_strikes >= 3 {
                profile.status = WalletClassification::BlacklistedDumper;
                return CopySignalVerdict::RevokeBookmarkAndBlacklist {
                    reason: "Confirmed Insider Cabal Group Coordinator",
                };
            }
            return CopySignalVerdict::SkipSuspiciousPlay {
                reason: "Synchronized Insider Group Entry Detected — Refusing to Copy",
            };
        }

        CopySignalVerdict::ApprovedCopy {
            recommended_scale_bps: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalingTier {
    InitialRiskActive,
    PrincipalRecovered2_5x,
    TenXMilestoneBanked,
    HundredXMilestoneBanked,
    ThousandXOutlierLocked,
    Closed,
}

#[derive(Debug, Clone)]
pub struct ActiveCopyPosition {
    pub mint_pubkey: String,
    pub copied_insider_wallet: String,
    pub entry_sol_amount: f64,
    pub realized_sol_returned: f64,
    pub entry_mcap_usd: f64,
    pub peak_mcap_usd: f64,
    pub remaining_tokens: u64,
    pub tier: ScalingTier,
}

#[derive(Debug, Clone)]
pub enum PositionAction {
    Hold { current_multiple: f64 },
    ScaleOut {
        sell_tokens: u64,
        sol_proceeds: f64,
        current_multiple: f64,
        new_tier: ScalingTier,
        reason: &'static str,
    },
}

impl ActiveCopyPosition {
    /// Original 2-argument tick evaluator preserved
    #[inline]
    pub fn evaluate_market_tick(
        &mut self,
        chart: &LiveChartMetrics,
        insider_sold: bool,
    ) -> PositionAction {
        self.evaluate_market_tick_full(chart, insider_sold, 1, 0)
    }

    /// Bonded Full Surveillance Evaluator: checks 2.5x->1000x scaling, insider exits,
    /// canary external sells (delayed honeypot check), and cabal dumps.
    #[inline]
    pub fn evaluate_market_tick_full(
        &mut self,
        chart: &LiveChartMetrics,
        insider_sold: bool,
        external_sells_succeeded: u32,
        cabal_wallets_dumping: u32,
    ) -> PositionAction {
        if self.tier == ScalingTier::Closed || self.remaining_tokens == 0 {
            return PositionAction::Hold { current_multiple: 0.0 };
        }

        let current_multiple = if self.entry_mcap_usd > 0.0 {
            chart.market_cap_usd / self.entry_mcap_usd
        } else {
            1.0
        };

        if chart.market_cap_usd > self.peak_mcap_usd {
            self.peak_mcap_usd = chart.market_cap_usd;
        }

        // 1. Live Delayed-Honeypot Canary Check
        if current_multiple >= 1.8 && external_sells_succeeded == 0 {
            let sold = self.remaining_tokens;
            let sol_out = self.entry_sol_amount * current_multiple * 0.4;
            self.realized_sol_returned += sol_out;
            self.remaining_tokens = 0;
            self.tier = ScalingTier::Closed;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: ScalingTier::Closed,
                reason: "HONEYPOT CANARY TRIGGERED: Price pumping with 0 external sells — Emergency Exit",
            };
        }

        // 2. Insider or Cabal Cluster Dump Defense
        if insider_sold || cabal_wallets_dumping >= 2 {
            let sold = self.remaining_tokens;
            let sol_out = self.entry_sol_amount * current_multiple * 0.5;
            self.realized_sol_returned += sol_out;
            self.remaining_tokens = 0;
            self.tier = ScalingTier::Closed;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: ScalingTier::Closed,
                reason: "Copied Insider / Cabal Group Sold — Immediate Frontrun Exit",
            };
        }

        // 3. Initial Hard Stop-Loss (-35%) while principal is at risk
        if self.tier == ScalingTier::InitialRiskActive && current_multiple <= 0.65 {
            let sold = self.remaining_tokens;
            let sol_out = self.entry_sol_amount * current_multiple;
            self.realized_sol_returned += sol_out;
            self.remaining_tokens = 0;
            self.tier = ScalingTier::Closed;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: ScalingTier::Closed,
                reason: "Initial Hard Stop-Loss (-35% Market Cap)",
            };
        }

        // 4. Adaptive Trailing High-Water-Mark Stop (30% at 2.5x-10x, tightens to 15% at 100x-1000x)
        if self.tier != ScalingTier::InitialRiskActive && self.peak_mcap_usd > 0.0 {
            let drawdown = (self.peak_mcap_usd - chart.market_cap_usd) / self.peak_mcap_usd;
            let max_allowed_drawdown = match self.tier {
                ScalingTier::HundredXMilestoneBanked | ScalingTier::ThousandXOutlierLocked => 0.15,
                _ => 0.30,
            };

            if drawdown >= max_allowed_drawdown {
                let sold = self.remaining_tokens;
                let sol_out = self.entry_sol_amount * current_multiple * 0.15;
                self.realized_sol_returned += sol_out;
                self.remaining_tokens = 0;
                self.tier = ScalingTier::Closed;
                return PositionAction::ScaleOut {
                    sell_tokens: sold,
                    sol_proceeds: sol_out,
                    current_multiple,
                    new_tier: ScalingTier::Closed,
                    reason: "Adaptive High-Water-Mark Trailing Stop Triggered",
                };
            }
        }

        // 5. Original 2.5x -> 10x -> 100x -> 1000x+ Moonbag Scaling Ladder
        if current_multiple >= 1000.0 && self.tier != ScalingTier::ThousandXOutlierLocked {
            let sold = self.remaining_tokens / 2;
            self.remaining_tokens -= sold;
            let sol_out = self.entry_sol_amount * 150.0;
            self.realized_sol_returned += sol_out;
            self.tier = ScalingTier::ThousandXOutlierLocked;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: self.tier,
                reason: "1000x OUTLIER HIT: Banked 50% of remaining moonbag",
            };
        }

        if current_multiple >= 100.0
            && (self.tier == ScalingTier::PrincipalRecovered2_5x
                || self.tier == ScalingTier::TenXMilestoneBanked)
        {
            let sold = self.remaining_tokens / 2;
            self.remaining_tokens -= sold;
            let sol_out = self.entry_sol_amount * 15.0;
            self.realized_sol_returned += sol_out;
            self.tier = ScalingTier::HundredXMilestoneBanked;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: self.tier,
                reason: "100x MILESTONE HIT: Banked 50% of remaining moonbag",
            };
        }

        if current_multiple >= 10.0 && self.tier == ScalingTier::PrincipalRecovered2_5x {
            let sold = self.remaining_tokens * 40 / 100;
            self.remaining_tokens -= sold;
            let sol_out = self.entry_sol_amount * 2.0;
            self.realized_sol_returned += sol_out;
            self.tier = ScalingTier::TenXMilestoneBanked;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: self.tier,
                reason: "10x MILESTONE HIT: Locked in profit, riding risk-free moonbag",
            };
        }

        if current_multiple >= 2.5 && self.tier == ScalingTier::InitialRiskActive {
            let sold = self.remaining_tokens / 2;
            self.remaining_tokens -= sold;
            let sol_out = self.entry_sol_amount * 1.25;
            self.realized_sol_returned += sol_out;
            self.tier = ScalingTier::PrincipalRecovered2_5x;
            return PositionAction::ScaleOut {
                sell_tokens: sold,
                sol_proceeds: sol_out,
                current_multiple,
                new_tier: self.tier,
                reason: "2.5x PRINCIPAL RECOVERED: Initial SOL + Jito Tips banked (RISK-FREE)",
            };
        }

        PositionAction::Hold { current_multiple }
    }
}