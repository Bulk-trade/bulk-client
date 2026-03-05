use serde::{Deserialize, Serialize};
use crate::msgs::{AgentWalletCreation, CancelAll, CancelOrder, Faucet, LimitOrder, MarketOrder, ModifyOrder, Price, PythOracle, UpdateUserSettings, WhitelistFaucet};

#[derive(Clone, Debug, Serialize, Deserialize)]
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

    #[serde(rename = "px")]
    Price(Price),
    #[serde(rename = "o")]
    PythOracle(PythOracle),

    #[serde(rename = "faucet")]
    Faucet(Faucet),
    #[serde(rename = "agentWalletCreation")]
    AgentWalletCreation(AgentWalletCreation),
    #[serde(rename = "updateUserSettings")]
    UpdateUserSettings(UpdateUserSettings),
    #[serde(rename = "whitelistFaucet")]
    WhitelistFaucet(WhitelistFaucet),
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