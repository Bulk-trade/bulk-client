use crate::msgs::conditional::{OnFill, Range, StopOrTP, Trailing, Trigger};
use crate::msgs::liquidator::LiqConfig;
use crate::msgs::multisig::{
    CreateMultisig, MultisigApprove, MultisigCancel, MultisigExecute, MultisigPropose,
    MultisigReject, UpdateMultisigPolicy,
};
use crate::msgs::risk::RiskConfigChange;
use crate::msgs::subaccounts::{CreateSubAccount, RemoveSubAccount, RenameSubAccount, Transfer};
use crate::msgs::{
    ActivateProtocolVersion, AddMarket, AgentWalletCreation, ApproveCommissionFee, Beacon,
    CancelAll, CancelOrder, ConfigFunding, ConfigMakerRebateTier, Deposit, DkgFinished, DkgRound1,
    Faucet, FrostWithdrawStart, InitializeVault, Join, LimitOrder, MarketAdmin, MarketOrder,
    Matrix, ModifyOrder, NonceCommitment, OpaqueAction, PartialSignature, PreDepositCredit, Price,
    PricingAdmin, PythOracle, RevokeCommissionFee, RevokePendingActivation, RewardSettlement,
    SolanaBlockAnchor, UpdateAccountPolicy, UpdateFrostGroup, UpdateUserSettings,
    UpdateValidatorSet, UserAdmin, WhitelistFaucet, Withdraw, WithdrawConfirmation, WithdrawFailed,
    WithdrawSubmitted,
};
use serde::ser::{SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use solana_pubkey::Pubkey;

/// Meta data for an action
#[derive(Clone, Copy, Debug, Default)]
pub struct ActionMeta {
    pub account: Pubkey,
    pub nonce: u64,
    pub seqno: u32,
    pub hash: Option<Hash>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    // Market = ordinal(0)
    #[serde(rename = "m")]
    MarketOrder(MarketOrder),
    // Limit = ordinal(1)
    #[serde(rename = "l")]
    LimitOrder(LimitOrder),
    // Modify = ordinal(2)
    #[serde(rename = "mod")]
    ModifyOrder(ModifyOrder),
    // Cancel = ordinal(3)
    #[serde(rename = "cx")]
    Cancel(CancelOrder),
    // CancelAll = ordinal(4)
    #[serde(rename = "cxa")]
    CancelAll(CancelAll),
    // Stop = ordinal(5)
    #[serde(rename = "st")]
    Stop(StopOrTP),
    // TakeProfit = ordinal(6)
    #[serde(rename = "tp")]
    TakeProfit(StopOrTP),
    // Range = ordinal(7)
    #[serde(rename = "rng")]
    Range(Range),
    // Trigger = ordinal(8)
    #[serde(rename = "trig")]
    Trigger(Trigger),
    // Trailing = ordinal(9)
    #[serde(rename = "trl")]
    Trailing(Trailing),
    // OnFill = ordinal(10)
    #[serde(rename = "of")]
    OnFill(OnFill),

    // Price = ordinal(11)
    #[serde(rename = "px")]
    Price(Price),
    // Corrs = ordinal(12)
    #[serde(rename = "corrs")]
    Corrs(Matrix),
    // PythOracle = ordinal(13)
    #[serde(rename = "o")]
    PythOracle(PythOracle),
    // Beacon = ordinal(14)
    #[serde(rename = "beacon")]
    Beacon(Beacon),
    // Join = ordinal(15)
    #[serde(rename = "join")]
    Join(Join),

    // Faucet = ordinal(16)
    Faucet(Faucet),
    // AgentWallet = ordinal(17)
    AgentWalletCreation(AgentWalletCreation),
    // UpdateUserSettings = ordinal(18)
    UpdateUserSettings(UpdateUserSettings),

    // WhitelistFaucet = ordinal(19)
    WhitelistFaucet(WhitelistFaucet),

    // AddMarket = ordinal(20)
    AddMarket(AddMarket),
    // ConfigFairPrice = ordinal(21)
    ConfigFairPrice(OpaqueAction),
    // ConfigVolatility = ordinal(22)
    ConfigVolatility(OpaqueAction),
    // ConfigSecurity = ordinal(23)
    ConfigSecurity(OpaqueAction),
    // ConfigRegime = ordinal(24)
    ConfigRegime(OpaqueAction),
    // ConfigRisk = ordinal(25)
    #[serde(alias = "configRiskMatrix")]
    ConfigRisk(OpaqueAction),
    // ConfigFeePolicy = ordinal(26)
    #[serde(rename = "cfgf")]
    ConfigFeePolicy(OpaqueAction),

    // CreateSubAccount = ordinal(27)
    CreateSubAccount(CreateSubAccount),
    // RemoveSubAccount = ordinal(28)
    RemoveSubAccount(RemoveSubAccount),
    // Transfer = ordinal(29)
    Transfer(Transfer),
    // CreateMultisig = ordinal(30)
    CreateMultisig(CreateMultisig),

    // MultisigPropose = ordinal(31)
    #[serde(rename = "msp")]
    MultisigPropose(MultisigPropose),
    // MultisigApprove = ordinal(32)
    #[serde(rename = "msa")]
    MultisigApprove(MultisigApprove),
    // MultisigReject = ordinal(33)
    #[serde(rename = "msr")]
    MultisigReject(MultisigReject),
    // MultisigCancel = ordinal(34)
    #[serde(rename = "msc")]
    MultisigCancel(MultisigCancel),
    // MultisigExecute = ordinal(35)
    #[serde(rename = "mse")]
    MultisigExecute(MultisigExecute),
    // UpdateMultisigPolicy = ordinal(36)
    #[serde(rename = "msu")]
    UpdateMultisigPolicy(UpdateMultisigPolicy),

    // RenameSubAccount = ordinal(37)
    #[serde(rename = "rsa", alias = "renameSubAccount")]
    RenameSubAccount(RenameSubAccount),
    // UpdateValidatorSet = ordinal(38)
    #[serde(rename = "uvs")]
    UpdateValidatorSet(UpdateValidatorSet),

    // UpdateRiskConfig = ordinal(39)
    #[serde(rename = "risk")]
    UpdateRiskConfig(RiskConfigChange),
    // ApproveCommissionFee = ordinal(40)
    #[serde(rename = "abc", alias = "approveBuilderCode")]
    ApproveCommissionFee(ApproveCommissionFee),
    // RevokeCommissionFee = ordinal(41)
    #[serde(rename = "rbc", alias = "revokeBuilderCode")]
    RevokeCommissionFee(RevokeCommissionFee),

    // RewardSettlement = ordinal(42)
    #[serde(rename = "rs")]
    RewardSettlement(RewardSettlement),

    // UpdateLiquidatorConfig = ordinal(43)
    #[serde(rename = "liq")]
    UpdateLiquidatorConfig(LiqConfig),

    // Deposit = ordinal(44)
    #[serde(rename = "deposit")]
    Deposit(Deposit),
    // Withdraw = ordinal(45)
    #[serde(rename = "withdraw")]
    Withdraw(Withdraw),
    // WithdrawConfirmation = ordinal(46)
    #[serde(rename = "WithdrawConfirmation")]
    WithdrawConfirmation(WithdrawConfirmation),
    // NonceCommitment = ordinal(47)
    #[serde(rename = "nonceCommitment")]
    NonceCommitment(NonceCommitment),
    // PartialSignature = ordinal(48)
    #[serde(rename = "partialSignature")]
    PartialSignature(PartialSignature),
    // WithdrawSubmitted = ordinal(49)
    #[serde(rename = "withdrawSubmitted")]
    WithdrawSubmitted(WithdrawSubmitted),
    // WithdrawFailed = ordinal(50)
    #[serde(rename = "withdrawFailed")]
    WithdrawFailed(WithdrawFailed),
    // DkgRound1 = ordinal(51)
    #[serde(rename = "dkgRound1")]
    DkgRound1(DkgRound1),
    // InitializeVault = ordinal(52)
    #[serde(rename = "initializeVault")]
    InitializeVault(InitializeVault),
    // UpdateFrostGroup = ordinal(53)
    #[serde(rename = "updateFrostGroup")]
    UpdateFrostGroup(UpdateFrostGroup),
    // DkgFinished = ordinal(54)
    #[serde(rename = "dkgFinished")]
    DkgFinished(DkgFinished),
    // SolanaBlockAnchor = ordinal(55)
    #[serde(rename = "solanaBlockAnchor")]
    SolanaBlockAnchor(SolanaBlockAnchor),
    // ConfigMakerRebateTier = ordinal(56)
    #[serde(rename = "cmrt", alias = "configMakerRebateTier")]
    ConfigMakerRebateTier(ConfigMakerRebateTier),

    // MarketAdmin = ordinal(57)
    #[serde(rename = "marketAdmin")]
    MarketAdmin(MarketAdmin),
    // PricingAdmin = ordinal(58)
    #[serde(rename = "pricingAdmin")]
    PricingAdmin(PricingAdmin),

    // FrostWithdrawStart = ordinal(59)
    #[serde(rename = "frostWithdrawStart")]
    FrostWithdrawStart(FrostWithdrawStart),
    // PreDepositCredit = ordinal(60)
    #[serde(rename = "preDepositCredit")]
    PreDepositCredit(PreDepositCredit),
    // UpdateAccountPolicy = ordinal(61)
    #[serde(rename = "accountPolicy", alias = "updateAccountPolicy")]
    UpdateAccountPolicy(UpdateAccountPolicy),

    // ActivateProtocolVersion = ordinal(62)
    #[serde(rename = "activateProtocolVersion")]
    ActivateProtocolVersion(ActivateProtocolVersion),
    // RevokePendingActivation = ordinal(63)
    #[serde(rename = "revokePendingActivation")]
    RevokePendingActivation(RevokePendingActivation),
    // ConfigFunding = ordinal(64)
    #[serde(rename = "configFunding")]
    ConfigFunding(ConfigFunding),

    // UserAdmin = ordinal(65)
    #[serde(rename = "userAdmin")]
    UserAdmin(UserAdmin),
}

macro_rules! dispatch {
    ($self:expr, $x:ident => $body:expr) => {
        match $self {
            Action::MarketOrder($x) => $body,
            Action::LimitOrder($x) => $body,
            Action::ModifyOrder($x) => $body,
            Action::Cancel($x) => $body,
            Action::CancelAll($x) => $body,
            Action::Stop($x) => $body,
            Action::TakeProfit($x) => $body,
            Action::Range($x) => $body,
            Action::Trigger($x) => $body,
            Action::Trailing($x) => $body,
            Action::OnFill($x) => $body,
            Action::Price($x) => $body,
            Action::Corrs($x) => $body,
            Action::PythOracle($x) => $body,
            Action::Beacon($x) => $body,
            Action::Join($x) => $body,

            Action::Faucet($x) => $body,
            Action::AgentWalletCreation($x) => $body,
            Action::UpdateUserSettings($x) => $body,
            Action::WhitelistFaucet($x) => $body,

            Action::AddMarket($x) => $body,
            Action::ConfigFairPrice($x) => $body,
            Action::ConfigVolatility($x) => $body,
            Action::ConfigSecurity($x) => $body,
            Action::ConfigRegime($x) => $body,
            Action::ConfigRisk($x) => $body,
            Action::ConfigFeePolicy($x) => $body,

            Action::CreateSubAccount($x) => $body,
            Action::RemoveSubAccount($x) => $body,
            Action::Transfer($x) => $body,

            Action::CreateMultisig($x) => $body,
            Action::MultisigPropose($x) => $body,
            Action::MultisigApprove($x) => $body,
            Action::MultisigReject($x) => $body,
            Action::MultisigCancel($x) => $body,
            Action::MultisigExecute($x) => $body,
            Action::UpdateMultisigPolicy($x) => $body,

            Action::RenameSubAccount($x) => $body,
            Action::UpdateValidatorSet($x) => $body,

            Action::UpdateRiskConfig($x) => $body,
            Action::ApproveCommissionFee($x) => $body,
            Action::RevokeCommissionFee($x) => $body,
            Action::RewardSettlement($x) => $body,
            Action::UpdateLiquidatorConfig($x) => $body,
            Action::Deposit($x) => $body,
            Action::Withdraw($x) => $body,
            Action::WithdrawConfirmation($x) => $body,
            Action::NonceCommitment($x) => $body,
            Action::PartialSignature($x) => $body,
            Action::WithdrawSubmitted($x) => $body,
            Action::WithdrawFailed($x) => $body,
            Action::DkgRound1($x) => $body,
            Action::InitializeVault($x) => $body,
            Action::UpdateFrostGroup($x) => $body,
            Action::DkgFinished($x) => $body,
            Action::SolanaBlockAnchor($x) => $body,
            Action::ConfigMakerRebateTier($x) => $body,
            Action::MarketAdmin($x) => $body,
            Action::PricingAdmin($x) => $body,
            Action::FrostWithdrawStart($x) => $body,
            Action::PreDepositCredit($x) => $body,
            Action::UpdateAccountPolicy($x) => $body,
            Action::ActivateProtocolVersion($x) => $body,
            Action::RevokePendingActivation($x) => $body,
            Action::ConfigFunding($x) => $body,
            Action::UserAdmin($x) => $body,
        }
    };
}

impl Action {
    /// Returns whether this action requires the configured administrative multisig.
    ///
    /// - Matches market, pricing, risk-model, and security administration.
    pub fn is_admin_multisig_action(&self) -> bool {
        matches!(
            self,
            Action::AddMarket(_)
                | Action::Corrs(_)
                | Action::PricingAdmin(_)
                | Action::MarketAdmin(_)
                | Action::ConfigRegime(_)
                | Action::ConfigSecurity(_)
                | Action::ConfigFairPrice(_)
                | Action::ConfigVolatility(_)
                | Action::ConfigRisk(_)
                | Action::UpdateRiskConfig(_)
                | Action::UpdateAccountPolicy(_)
                | Action::ConfigFunding(_)
                | Action::UserAdmin(_)
        )
    }

    /// Returns whether this action requires the configured fee-administration multisig.
    pub fn is_fee_admin_multisig_action(&self) -> bool {
        matches!(
            self,
            Action::ConfigFeePolicy(_) | Action::ConfigMakerRebateTier(_)
        )
    }

    /// Get account associated with action
    pub fn account(&self) -> &Pubkey {
        dispatch!(self, x => &x.meta.account)
    }

    /// Get nonce associated with action
    pub fn nonce(&self) -> u64 {
        dispatch!(self, x => x.meta.nonce)
    }

    /// Get nonce associated with action
    pub fn seqno(&self) -> u32 {
        dispatch!(self, x => x.meta.seqno)
    }

    /// Get or compute hash of action
    pub fn hash(&mut self) -> Hash {
        use sha2::Digest;

        // Single dispatch: cache check + extract raw pointer to meta
        let meta_ptr: *mut ActionMeta = dispatch!(self, x => {
            if let Some(h) = x.meta.hash {
                return h;
            }
            &mut x.meta as *mut ActionMeta
        });

        // Read meta fields through pointer — no live borrows of self
        let (seqno, account, nonce) = unsafe {
            let m = &*meta_ptr;
            (m.seqno, m.account, m.nonce)
        };

        let mut hasher = sha2::Sha256::new();
        hasher.update(&seqno.to_le_bytes());
        bincode::serialize_into(&mut hasher, &OrderHashAction(&*self))
            .expect("serialization failed");
        // Immutable borrow of self released here
        hasher.update(account.as_ref());
        hasher.update(&nonce.to_le_bytes());

        let hash = Hash::from(Into::<[u8; 32]>::into(hasher.finalize()));

        // Write result — no active borrows of self
        unsafe {
            (*meta_ptr).hash = Some(hash);
        }
        hash
    }

    /// Link tx and action meta information in each action
    pub fn link(&mut self, meta: ActionMeta) {
        dispatch!(self, x => {
            x.meta = meta;
        })
    }
}

impl From<MarketOrder> for Action {
    fn from(o: MarketOrder) -> Self {
        Action::MarketOrder(o)
    }
}

impl From<LimitOrder> for Action {
    fn from(o: LimitOrder) -> Self {
        Action::LimitOrder(o)
    }
}

struct OrderHashSafeF64(f64);

impl Serialize for OrderHashSafeF64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::msgs::fixed_point::serialize(&self.0, serializer)
    }
}

struct OrderHashMarketOrder<'a>(&'a MarketOrder);

impl Serialize for OrderHashMarketOrder<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(5)?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        tuple.end()
    }
}

struct OrderHashLimitOrder<'a>(&'a LimitOrder);

impl Serialize for OrderHashLimitOrder<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(7)?;
        tuple.serialize_element(&self.0.symbol)?;
        tuple.serialize_element(&self.0.is_buy)?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.price))?;
        tuple.serialize_element(&OrderHashSafeF64(self.0.size))?;
        tuple.serialize_element(&self.0.tif)?;
        tuple.serialize_element(&self.0.reduce_only)?;
        tuple.serialize_element(&self.0.iso)?;
        tuple.end()
    }
}

struct OrderHashAction<'a>(&'a Action);

impl Serialize for OrderHashAction<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Action::MarketOrder(order) => serializer.serialize_newtype_variant(
                "Action",
                0,
                "MarketOrder",
                &OrderHashMarketOrder(order),
            ),
            Action::LimitOrder(order) => serializer.serialize_newtype_variant(
                "Action",
                1,
                "LimitOrder",
                &OrderHashLimitOrder(order),
            ),
            action => action.serialize(serializer),
        }
    }
}

impl From<ModifyOrder> for Action {
    fn from(o: ModifyOrder) -> Self {
        Action::ModifyOrder(o)
    }
}

impl From<CancelAll> for Action {
    fn from(o: CancelAll) -> Self {
        Action::CancelAll(o)
    }
}

impl From<CancelOrder> for Action {
    fn from(o: CancelOrder) -> Self {
        Action::Cancel(o)
    }
}

impl From<Price> for Action {
    fn from(o: Price) -> Self {
        Action::Price(o)
    }
}

impl From<PythOracle> for Action {
    fn from(o: PythOracle) -> Self {
        Action::PythOracle(o)
    }
}

impl From<Faucet> for Action {
    fn from(o: Faucet) -> Self {
        Action::Faucet(o)
    }
}

impl From<AgentWalletCreation> for Action {
    fn from(o: AgentWalletCreation) -> Self {
        Action::AgentWalletCreation(o)
    }
}

impl From<UpdateUserSettings> for Action {
    fn from(o: UpdateUserSettings) -> Self {
        Action::UpdateUserSettings(o)
    }
}

impl From<ApproveCommissionFee> for Action {
    fn from(o: ApproveCommissionFee) -> Self {
        Action::ApproveCommissionFee(o)
    }
}

impl From<RevokeCommissionFee> for Action {
    fn from(o: RevokeCommissionFee) -> Self {
        Action::RevokeCommissionFee(o)
    }
}

impl From<WhitelistFaucet> for Action {
    fn from(o: WhitelistFaucet) -> Self {
        Action::WhitelistFaucet(o)
    }
}

impl From<MarketAdmin> for Action {
    fn from(o: MarketAdmin) -> Self {
        Action::MarketAdmin(o)
    }
}

macro_rules! impl_action_from {
    ($type:ty, $variant:ident) => {
        impl From<$type> for Action {
            fn from(action: $type) -> Self {
                Action::$variant(action)
            }
        }
    };
}

impl_action_from!(RewardSettlement, RewardSettlement);
impl_action_from!(Deposit, Deposit);
impl_action_from!(Withdraw, Withdraw);
impl_action_from!(WithdrawConfirmation, WithdrawConfirmation);
impl_action_from!(NonceCommitment, NonceCommitment);
impl_action_from!(PartialSignature, PartialSignature);
impl_action_from!(WithdrawSubmitted, WithdrawSubmitted);
impl_action_from!(WithdrawFailed, WithdrawFailed);
impl_action_from!(DkgRound1, DkgRound1);
impl_action_from!(InitializeVault, InitializeVault);
impl_action_from!(UpdateFrostGroup, UpdateFrostGroup);
impl_action_from!(DkgFinished, DkgFinished);
impl_action_from!(SolanaBlockAnchor, SolanaBlockAnchor);
impl_action_from!(ConfigMakerRebateTier, ConfigMakerRebateTier);
impl_action_from!(PricingAdmin, PricingAdmin);
impl_action_from!(FrostWithdrawStart, FrostWithdrawStart);
impl_action_from!(PreDepositCredit, PreDepositCredit);
impl_action_from!(UpdateAccountPolicy, UpdateAccountPolicy);
impl_action_from!(ActivateProtocolVersion, ActivateProtocolVersion);
impl_action_from!(RevokePendingActivation, RevokePendingActivation);
impl_action_from!(ConfigFunding, ConfigFunding);
impl_action_from!(UserAdmin, UserAdmin);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tif::TimeInForce;
    use crate::msgs::OracleSource;
    use std::sync::Arc;

    #[test]
    fn test_limit_hash() {
        let limit = LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100000.0,
            size: 1.0,
            tif: TimeInForce::ALO,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta: ActionMeta {
                account: Default::default(),
                nonce: 1_776_128_000_000_000_000,
                seqno: 0,
                hash: None,
            },
        };

        let mut action = Action::LimitOrder(limit);
        let hash = action.hash();

        assert_eq!(
            hash.to_string(),
            "9BreqftLa7ZAsYLkvJDRBRxiSukzGoTfbQNMBWWkUAUJ"
        );
    }

    #[test]
    fn pricing_admin_uses_wire_ordinal_58_and_camel_case_json() {
        let action = Action::PricingAdmin(PricingAdmin {
            instrument: Arc::from("BTC"),
            source: OracleSource::Bulk,
            meta: ActionMeta::default(),
        });

        let encoded = bincode::serialize(&action).expect("serialize pricing admin");
        assert_eq!(&encoded[..4], &58_u32.to_le_bytes());
        assert_eq!(
            serde_json::to_value(action).expect("serialize pricing admin JSON"),
            serde_json::json!({
                "pricingAdmin": {
                    "instrument": "BTC",
                    "source": "bulk"
                }
            })
        );
    }

    #[test]
    fn order_hash_ignores_commission() {
        let meta = ActionMeta {
            account: Pubkey::new_unique(),
            nonce: 7,
            seqno: 3,
            hash: None,
        };
        let mut without = Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100000.0,
            size: 1.0,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            builder_code: None,
            meta,
        });
        let mut with = Action::LimitOrder(LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100000.0,
            size: 1.0,
            tif: TimeInForce::GTC,
            reduce_only: false,
            iso: false,
            builder_code: Some(crate::msgs::BuilderCode {
                to: Pubkey::new_unique(),
                fee: 5,
            }),
            meta,
        });

        assert_eq!(without.hash(), with.hash());
    }

    #[test]
    fn config_risk_accepts_risk_matrix_alias() {
        let action: Action = serde_json::from_str(r#"{"configRiskMatrix":{"payload":[1,2,3]}}"#)
            .expect("configRiskMatrix alias should deserialize");

        match action {
            Action::ConfigRisk(action) => assert_eq!(action.payload, vec![1, 2, 3]),
            action => panic!("expected ConfigRisk, got {action:?}"),
        }
    }

    #[test]
    fn market_admin_uses_sdk_json_shape() {
        let action = Action::MarketAdmin(MarketAdmin {
            symbol: Arc::from("BTC-USD"),
            action: crate::msgs::MarketAction::Suspend,
            price: Some(100_000.5),
            meta: ActionMeta::default(),
        });

        let json = serde_json::to_value(&action).expect("market admin should serialize");

        assert_eq!(
            json,
            serde_json::json!({
                "marketAdmin": {
                    "symbol": "BTC-USD",
                    "action": "suspend",
                    "price": "100000.5"
                }
            })
        );
    }

    #[test]
    fn config_funding_uses_sdk_ordinal_and_admin_multisig() {
        let action = Action::ConfigFunding(ConfigFunding {
            symbol: "BTC-USD".to_owned(),
            rate: 0.125,
            deviation_cap: 0.0005,
            funding_cap: 0.004,
            premium_horizon: 3_600.0,
            notional: 100_000.0,
            sample_period: 1,
            mean_window: 3_600,
            meta: ActionMeta::default(),
        });

        let bytes = bincode::serialize(&action).expect("funding config should serialize");

        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 64);
        assert!(action.is_admin_multisig_action());
    }
}
