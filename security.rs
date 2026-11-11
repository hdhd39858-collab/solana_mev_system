use serde::{Deserialize, Serialize};

pub const PUMPFUN_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const JITO_DONT_FRONT_ACCOUNT: &str = "jitodontfront111111111111111111111111111111";
pub const JITO_TIP_ACCOUNT_0: &str = "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5";

pub const IX_PUMPFUN_CREATE: [u8; 8] = [0x18, 0x1e, 0xc8, 0x28, 0x05, 0x1c, 0x07, 0x77];
pub const IX_PUMPFUN_BUY: [u8; 8]    = [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea];
pub const IX_PUMPFUN_SELL: [u8; 8]   = [0x33, 0xe6, 0x85, 0xa4, 0x01, 0x7f, 0x83, 0xad];

pub const LAMPORTS_PER_SOL: u128 = 1_000_000_000;
pub const BPS_DENOMINATOR: u128 = 10_000;
pub const PUMPFUN_FEE_BPS: u128 = 100; // 1.00% protocol fee
pub const PUMPFUN_GRADUATION_SOL_LAMPORTS: u64 = 85_000_000_000; // ~85 SOL migrates to Raydium/PumpSwap

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolanaThreatVerdict {
    Clean,
    MalformedAccountSize,
    MintAuthorityNotRevoked,
    FreezeAuthorityActive,
    Token2022MaliciousExtension,
    CreatorDevSupplyExceedsThreshold { dev_share_bps: u128 },
    CreatorDevSupplyExceedsRegime { dev_share_bps: u128, regime_limit_bps: u128 },
    StealthHoneypotNoExternalSells,
}

/// Three-tier decision state: allows the bot to pause and observe suspicious plays
/// rather than blindly buying or permanently blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityDecision {
    ClearForImmediateEntry { confidence_bps: u16 },
    HoldAndObserve { wait_slots: u64, reason: &'static str },
    HardQuarantine(SolanaThreatVerdict),
}

/// Original Zero-Copy 82-byte SPL Mint Inspector
#[derive(Debug, Clone, Copy)]
pub struct ZeroCopySplMintView<'a> {
    raw: &'a [u8],
}

impl<'a> ZeroCopySplMintView<'a> {
    #[inline(always)]
    pub fn try_from_slice(data: &'a [u8]) -> Result<Self, SolanaThreatVerdict> {
        if data.len() < 82 {
            return Err(SolanaThreatVerdict::MalformedAccountSize);
        }
        Ok(Self { raw: data })
    }

    #[inline(always)]
    pub fn has_mint_authority(&self) -> bool {
        u32::from_le_bytes([self.raw[0], self.raw[1], self.raw[2], self.raw[3]]) != 0
    }

    #[inline(always)]
    pub fn has_freeze_authority(&self) -> bool {
        u32::from_le_bytes([self.raw[46], self.raw[47], self.raw[48], self.raw[49]]) != 0
    }

    #[inline(always)]
    pub fn has_untrusted_token2022_extensions(&self) -> bool {
        self.raw.len() > 82
    }

    #[inline(always)]
    pub fn verify_security_invariants(&self) -> Result<(), SolanaThreatVerdict> {
        if self.has_mint_authority() {
            return Err(SolanaThreatVerdict::MintAuthorityNotRevoked);
        }
        if self.has_freeze_authority() {
            return Err(SolanaThreatVerdict::FreezeAuthorityActive);
        }
        if self.has_untrusted_token2022_extensions() {
            return Err(SolanaThreatVerdict::Token2022MaliciousExtension);
        }
        Ok(())
    }
}

/// Original 49-byte Pump.fun Bonding Curve State & Constant-Product Math
#[derive(Debug, Clone, Copy)]
pub struct BondingCurveState {
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
}

impl BondingCurveState {
    #[inline(always)]
    pub fn try_from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 49 {
            return None;
        }
        Some(Self {
            virtual_token_reserves: u64::from_le_bytes(data[8..16].try_into().ok()?),
            virtual_sol_reserves: u64::from_le_bytes(data[16..24].try_into().ok()?),
            real_token_reserves: u64::from_le_bytes(data[24..32].try_into().ok()?),
            real_sol_reserves: u64::from_le_bytes(data[32..40].try_into().ok()?),
            token_total_supply: u64::from_le_bytes(data[40..48].try_into().ok()?),
            complete: data[48] != 0,
        })
    }

    #[inline(always)]
    pub const fn quote_buy_exact_in(&self, lamports_in: u64) -> u64 {
        if self.complete || lamports_in == 0 || self.virtual_sol_reserves == 0 {
            return 0;
        }
        let dx = lamports_in as u128;
        let net_dx = (dx * (BPS_DENOMINATOR - PUMPFUN_FEE_BPS)) / BPS_DENOMINATOR;
        let x = self.virtual_sol_reserves as u128;
        let y = self.virtual_token_reserves as u128;

        let tokens_out = net_dx.saturating_mul(y) / x.saturating_add(net_dx);
        if tokens_out > self.real_token_reserves as u128 {
            self.real_token_reserves
        } else {
            tokens_out as u64
        }
    }

    #[inline(always)]
    pub const fn calculate_price_impact_bps(&self, lamports_in: u64) -> u128 {
        if self.virtual_sol_reserves == 0 {
            return BPS_DENOMINATOR;
        }
        let dx = lamports_in as u128;
        let x = self.virtual_sol_reserves as u128;
        (dx * BPS_DENOMINATOR) / (x + dx)
    }
}

/// Restored Jito Atomic Bundle & Anti-MEV `jitodontfront` Builder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoAtomicBundleSpec {
    pub target_mint: String,
    pub buy_amount_lamports: u64,
    pub max_sol_cost_lamports: u64,
    pub expected_tokens_out: u64,
    pub jito_tip_lamports: u64,
    pub read_only_guard_account: &'static str,
}

impl JitoAtomicBundleSpec {
    #[inline(always)]
    pub fn build_protected_snipe(
        target_mint: String,
        curve: &BondingCurveState,
        buy_amount_lamports: u64,
        max_slippage_bps: u128,
        competing_priority_lamports: u64,
    ) -> Self {
        let expected_tokens_out = curve.quote_buy_exact_in(buy_amount_lamports);
        let max_sol_cost_lamports = ((buy_amount_lamports as u128)
            * (BPS_DENOMINATOR + max_slippage_bps)
            / BPS_DENOMINATOR) as u64;

        let dynamic_tip = competing_priority_lamports
            .saturating_mul(120)
            .checked_div(100)
            .unwrap_or(1_000_000)
            .max(1_000_000);

        Self {
            target_mint,
            buy_amount_lamports,
            max_sol_cost_lamports,
            expected_tokens_out,
            jito_tip_lamports: dynamic_tip,
            read_only_guard_account: JITO_DONT_FRONT_ACCOUNT,
        }
    }
}

/// Bonded Mathematical "Chart" Engine (Market Cap + Price Per Million + Graduation %)
#[derive(Debug, Clone, Copy)]
pub struct LiveChartMetrics {
    pub price_lamports_per_m_tokens: u128,
    pub market_cap_sol: f64,
    pub market_cap_usd: f64,
    pub bonding_curve_progress_bps: u128,
}

impl LiveChartMetrics {
    #[inline(always)]
    pub fn from_curve(curve: &BondingCurveState, sol_usd_price: f64) -> Self {
        if curve.virtual_token_reserves == 0 {
            return Self {
                price_lamports_per_m_tokens: 0,
                market_cap_sol: 0.0,
                market_cap_usd: 0.0,
                bonding_curve_progress_bps: 0,
            };
        }

        let mcap_lamports = ((curve.virtual_sol_reserves as u128)
            .saturating_mul(curve.token_total_supply as u128))
            / (curve.virtual_token_reserves as u128);

        let market_cap_sol = (mcap_lamports as f64) / (LAMPORTS_PER_SOL as f64);
        let market_cap_usd = market_cap_sol * sol_usd_price;
        let progress_bps = ((curve.real_sol_reserves as u128) * BPS_DENOMINATOR)
            / (PUMPFUN_GRADUATION_SOL_LAMPORTS as u128);
        let price_per_m = ((curve.virtual_sol_reserves as u128) * 1_000_000_000_000)
            / (curve.virtual_token_reserves as u128);

        Self {
            price_lamports_per_m_tokens: price_per_m,
            market_cap_sol,
            market_cap_usd,
            bonding_curve_progress_bps: progress_bps.min(BPS_DENOMINATOR),
        }
    }
}

/// Bonded Adaptive Market Regime Engine: learns from market rug waves and adjusts thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveSecurityEngine {
    pub rolling_rug_rate_bps: u128,
    pub dynamic_max_dev_share_bps: u128,
    pub dynamic_max_price_impact_bps: u128,
    pub min_organic_sells_required: u32,
    pub total_launches_sampled: u64,
}

impl AdaptiveSecurityEngine {
    pub fn new() -> Self {
        Self {
            rolling_rug_rate_bps: 5_000,
            dynamic_max_dev_share_bps: 1_800, // 18.0% initial cap
            dynamic_max_price_impact_bps: 400,// 4.0% max self-impact
            min_organic_sells_required: 2,
            total_launches_sampled: 0,
        }
    }

    pub fn record_market_regime_outcome(&mut self, token_rugged_or_trapped: bool) {
        self.total_launches_sampled += 1;
        let outcome_bps: u128 = if token_rugged_or_trapped { 10_000 } else { 0 };
        self.rolling_rug_rate_bps = (self.rolling_rug_rate_bps * 85 + outcome_bps * 15) / 100;

        if self.rolling_rug_rate_bps > 7_000 {
            self.dynamic_max_dev_share_bps = 1_000;
            self.dynamic_max_price_impact_bps = 250;
            self.min_organic_sells_required = 4;
        } else if self.rolling_rug_rate_bps < 4_000 {
            self.dynamic_max_dev_share_bps = 2_000;
            self.dynamic_max_price_impact_bps = 450;
            self.min_organic_sells_required = 1;
        }
    }

    #[inline]
    pub fn evaluate_play(
        &self,
        mint_bytes: &[u8],
        curve: &BondingCurveState,
        dev_initial_tokens: u64,
        same_slot_cabal_buys: u32,
        external_sells_observed: u32,
    ) -> SecurityDecision {
        let mint_view = match ZeroCopySplMintView::try_from_slice(mint_bytes) {
            Ok(v) => v,
            Err(verdict) => return SecurityDecision::HardQuarantine(verdict),
        };
        if let Err(verdict) = mint_view.verify_security_invariants() {
            return SecurityDecision::HardQuarantine(verdict);
        }

        if curve.token_total_supply > 0 {
            let dev_share_bps = ((dev_initial_tokens as u128) * BPS_DENOMINATOR)
                / (curve.token_total_supply as u128);

            if dev_share_bps > self.dynamic_max_dev_share_bps {
                return SecurityDecision::HardQuarantine(
                    SolanaThreatVerdict::CreatorDevSupplyExceedsRegime {
                        dev_share_bps,
                        regime_limit_bps: self.dynamic_max_dev_share_bps,
                    },
                );
            }

            if dev_share_bps > (self.dynamic_max_dev_share_bps * 70) / 100 {
                return SecurityDecision::HoldAndObserve {
                    wait_slots: 12,
                    reason: "Borderline Dev Supply — Observing for Insider Dump",
                };
            }
        }

        if same_slot_cabal_buys >= 3 {
            return SecurityDecision::HoldAndObserve {
                wait_slots: 20,
                reason: "Suspicious Multi-Wallet Cabal Bundle — Waiting for Organic Flow",
            };
        }

        if curve.real_sol_reserves > 5_000_000_000 && external_sells_observed == 0 {
            return SecurityDecision::HoldAndObserve {
                wait_slots: 10,
                reason: "Zero External Sells Observed Despite Pump — Testing Honeypot Risk",
            };
        }

        let confidence = (10_000u128.saturating_sub(self.rolling_rug_rate_bps / 2)) as u16;
        SecurityDecision::ClearForImmediateEntry {
            confidence_bps: confidence,
        }
    }
}