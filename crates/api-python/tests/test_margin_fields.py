from bulk_api.messages.account import Margin, MarginUpdate


NEW_MARGIN = {
    "totalMargin": 771.15,
    "availableMargin": 230.22,
    "executionImpact": -179.22,
    "transferableBalance": 0.0,
    "marginUsed": 540.93,
    "marginBufferRate": 0.05,
    "bufferedMargin": 567.98,
    "notional": 21_260.96,
    "realizedPnl": -179.90,
    "unrealizedPnl": -23.72,
    "fees": -42.43,
    "funding": -6.52,
}


def test_margin_parses_new_api_fields():
    margin = Margin.from_api(NEW_MARGIN)

    assert margin.total_margin == 771.15
    assert margin.available_margin == 230.22
    assert margin.execution_impact == -179.22
    assert margin.transferable_balance == 0.0
    assert margin.margin_buffer_rate == 0.05
    assert margin.buffered_margin == 567.98


def test_margin_update_parses_new_api_fields():
    margin = MarginUpdate.from_api(NEW_MARGIN)

    assert margin.total_margin == 771.15
    assert margin.available_margin == 230.22
    assert margin.execution_impact == -179.22
    assert margin.transferable_balance == 0.0


def test_margin_accepts_legacy_balance_names():
    margin = Margin.from_api({"totalBalance": 100.0, "availableBalance": 90.0})

    assert margin.total_margin == 100.0
    assert margin.available_margin == 90.0
    assert margin.execution_impact == 0.0
    assert margin.transferable_balance == 0.0
