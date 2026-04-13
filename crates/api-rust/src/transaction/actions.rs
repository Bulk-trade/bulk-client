use serde::{Deserialize, Serialize};
use solana_hash::Hash;
use solana_keypair::Pubkey;
use crate::msgs::{AddMarket, AgentWalletCreation, Beacon, CancelAll, CancelOrder, Faucet, Join, LimitOrder, MarketOrder, Matrix, ModifyOrder, OpaqueAction, Price, PythOracle, UpdateUserSettings, WhitelistFaucet};
use crate::msgs::conditional::{Range, StopOrTP, Trigger};

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
    #[serde(rename = "m")]
    MarketOrder(MarketOrder),
    #[serde(rename = "l")]
    LimitOrder(LimitOrder),
    #[serde(rename = "mod")]
    ModifyOrder(ModifyOrder),
    #[serde(rename = "cx")]
    Cancel(CancelOrder),
    #[serde(rename = "cxa")]
    CancelAll(CancelAll),
    #[serde(rename = "st")]
    Stop(StopOrTP),
    #[serde(rename = "tp")]
    TakeProfit(StopOrTP),
    #[serde(rename = "rng")]
    Range(Range),
    #[serde(rename = "trig")]
    Trigger(Trigger),

    #[serde(rename = "px")]
    Price(Price),
    #[serde(rename = "corrs")]
    Corrs(Matrix),
    #[serde(rename = "o")]
    PythOracle(PythOracle),
    Beacon(Beacon),
    Join(Join),

    Faucet(Faucet),
    AgentWalletCreation(AgentWalletCreation),
    UpdateUserSettings(UpdateUserSettings),
    WhitelistFaucet(WhitelistFaucet),

    AddMarket(AddMarket),
    ConfigFairPrice(OpaqueAction),
    ConfigVolatility(OpaqueAction),
    ConfigSecurity(OpaqueAction),
    ConfigRegime(OpaqueAction),
    ConfigRisk(OpaqueAction),
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
        }
    };
}

impl Action {
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
        bincode::serialize_into(&mut hasher, &*self).expect("serialization failed");
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

impl From<WhitelistFaucet> for Action {
    fn from(o: WhitelistFaucet) -> Self {
        Action::WhitelistFaucet(o)
    }
}



#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use crate::common::tif::TimeInForce;
    use super::*;

    #[test]
    fn test_limit_hash() {
        let limit = LimitOrder {
            symbol: Arc::from("BTC-USD"),
            is_buy: true,
            price: 100000.0,
            size: 1.0,
            tif: TimeInForce::ALO,
            reduce_only: false,
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
            "82GJrc8KoJYSPQfRH2T7B2L4eMZ3PLitdi1h7kuXMmMc"
        );
    }
}