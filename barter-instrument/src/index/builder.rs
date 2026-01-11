use crate::{
    Keyed,
    asset::{Asset, AssetIndex, ExchangeAsset},
    exchange::{ExchangeId, ExchangeIndex},
    index::{
        IndexedInstruments, find_asset_by_exchange_and_name_internal, find_exchange_by_exchange_id,
    },
    instrument::{Instrument, InstrumentIndex, spec::OrderQuantityUnits},
};

/// 用于增量构建 `IndexedInstruments` 的构建器。
/// 在交易系统中，我们通常先收集所有的交易工具（Instrument），
/// 然后再统一进行去重、排序，并生成连续的内存索引（Index）。
#[derive(Debug, Default)]
pub struct IndexedInstrumentsBuilder {
    /// 暂存所有涉及的交易所 ID。
    exchanges: Vec<ExchangeId>,
    /// 暂存所有原始交易工具数据。
    instruments: Vec<Instrument<ExchangeId, Asset>>,
    /// 暂存所有涉及的交易所资产。
    assets: Vec<ExchangeAsset<Asset>>,
}

impl IndexedInstrumentsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 向构建器中添加一个新的交易工具。
    /// ### 参数：
    /// * `instrument`: 原始交易工具数据（使用 `ExchangeId` 和 `Asset` 标识）。
    pub fn add_instrument(mut self, instrument: Instrument<ExchangeId, Asset>) -> Self {
        // 记录关联的交易所
        self.exchanges.push(instrument.exchange);

        // 记录关联的基础资产 (Base Asset)
        self.assets.push(ExchangeAsset::new(
            instrument.exchange,
            instrument.underlying.base.clone(),
        ));

        // 记录关联的计价资产 (Quote Asset)
        self.assets.push(ExchangeAsset::new(
            instrument.exchange,
            instrument.underlying.quote.clone(),
        ));

        // 如果是永续合约等复杂品种，还需记录结算资产 (Settlement Asset)
        if let Some(settlement_asset) = instrument.kind.settlement_asset() {
            self.assets.push(ExchangeAsset::new(
                instrument.exchange,
                settlement_asset.clone(),
            ));
        }

        // 处理特定的下单规则资产（例如某些交易所要求以特定资产计价）
        if let Some(spec) = instrument.spec.as_ref()
            && let OrderQuantityUnits::Asset(asset) = &spec.quantity.unit
        {
            self.assets
                .push(ExchangeAsset::new(instrument.exchange, asset.clone()));
        }

        // 暂存交易工具本身
        self.instruments.push(instrument);

        self
    }

    /// 执行构建逻辑：去重、排序，并将原始标识符转换为内存索引。
    pub fn build(mut self) -> IndexedInstruments {
        // 1. 去重与排序：确保相同的数据只占用一个索引位，且顺序稳定
        self.exchanges.sort();
        self.exchanges.dedup();
        self.instruments.sort();
        self.instruments.dedup();
        self.assets.sort();
        self.assets.dedup();

        // 2. 交易所索引化：为每个 ExchangeId 分配 ExchangeIndex
        let exchanges = self
            .exchanges
            .into_iter()
            .enumerate()
            .map(|(index, exchange)| Keyed::new(ExchangeIndex::new(index), exchange))
            .collect::<Vec<_>>();

        // 3. 资产索引化：为每个交易所资产分配 AssetIndex
        let assets = self
            .assets
            .into_iter()
            .enumerate()
            .map(|(index, exchange_asset)| Keyed::new(AssetIndex::new(index), exchange_asset))
            .collect::<Vec<_>>();

        // 4. 交易工具索引化：将 Instrument 内部的 ExchangeId/Asset 映射为对应的 Index
        let instruments = self
            .instruments
            .into_iter()
            .enumerate()
            .map(|(index, instrument)| {
                let exchange_id = instrument.exchange;
                let exchange_key = find_exchange_by_exchange_id(&exchanges, &exchange_id)
                    .expect("every exchange related to every instrument has been added");

                let instrument = instrument.map_exchange_key(Keyed::new(exchange_key, exchange_id));

                let instrument = instrument
                    .map_asset_key_with_lookup(|asset: &Asset| {
                        find_asset_by_exchange_and_name_internal(
                            &assets,
                            exchange_id,
                            &asset.name_internal,
                        )
                    })
                    .expect("every asset related to every instrument has been added");

                Keyed::new(InstrumentIndex::new(index), instrument)
            })
            .collect();

        IndexedInstruments {
            exchanges,
            instruments,
            assets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Underlying,
        instrument::{
            kind::InstrumentKind,
            name::{InstrumentNameExchange, InstrumentNameInternal},
            quote::InstrumentQuoteAsset,
            spec::{
                InstrumentSpec, InstrumentSpecNotional, InstrumentSpecPrice, InstrumentSpecQuantity,
            },
        },
        test_utils::{exchange_asset, instrument},
    };
    use rust_decimal_macros::dec;

    #[test]
    fn test_builder_basic_spot() {
        // Add single spot instrument
        let indexed = IndexedInstrumentsBuilder::default()
            .add_instrument(instrument(ExchangeId::BinanceSpot, "btc", "usdt"))
            .build();

        // Verify state
        assert_eq!(indexed.exchanges().len(), 1);
        assert_eq!(indexed.assets().len(), 2); // BTC and USDT
        assert_eq!(indexed.instruments().len(), 1);

        // Verify exchanges indexes
        assert_eq!(indexed.exchanges()[0].value, ExchangeId::BinanceSpot);

        // Verify asset indexes
        assert_eq!(
            indexed.assets()[0].value,
            exchange_asset(ExchangeId::BinanceSpot, "btc"),
        );
        assert_eq!(
            indexed.assets()[1].value,
            exchange_asset(ExchangeId::BinanceSpot, "usdt"),
        );

        // Very instrument indexes
        assert_eq!(
            indexed.instruments()[0].value,
            Instrument {
                exchange: Keyed::new(ExchangeIndex(0), ExchangeId::BinanceSpot),
                name_exchange: InstrumentNameExchange::new("btc_usdt"),
                name_internal: InstrumentNameInternal::new("binance_spot-btc_usdt"),
                underlying: Underlying {
                    base: AssetIndex(0),
                    quote: AssetIndex(1),
                },
                quote: InstrumentQuoteAsset::UnderlyingQuote,
                kind: InstrumentKind::Spot,
                spec: None
            }
        );
    }

    #[test]
    fn test_builder_deduplication() {
        // Add same spot instrument twice
        let indexed = IndexedInstrumentsBuilder::default()
            .add_instrument(instrument(ExchangeId::BinanceSpot, "BTC", "USDT"))
            .add_instrument(instrument(ExchangeId::BinanceSpot, "BTC", "USDT"))
            .build();

        // Should deduplicate exchanges and assets, but not instruments
        assert_eq!(indexed.exchanges().len(), 1); // Exchange are de-duped
        assert_eq!(indexed.assets().len(), 2); // BTC and USDT and de-duped
        assert_eq!(indexed.instruments().len(), 1); // Instruments are de-duped
    }

    #[test]
    fn test_builder_multiple_exchanges() {
        // Add instruments from different exchanges
        let indexed = IndexedInstrumentsBuilder::default()
            .add_instrument(instrument(ExchangeId::BinanceSpot, "BTC", "USDT"))
            .add_instrument(instrument(ExchangeId::Coinbase, "BTC", "USD"))
            .build();

        // Should maintain separate indices for same asset on different exchanges
        assert_eq!(indexed.exchanges().len(), 2);
        assert_eq!(indexed.assets().len(), 4); // BTC on both exchanges, USDT and USD
        assert_eq!(indexed.instruments().len(), 2);
    }

    #[test]
    fn test_builder_asset_unit_handling() {
        // Create instrument with asset-based order quantity
        let base_asset = Asset::new_from_exchange("BTC");
        let quote_asset = Asset::new_from_exchange("USDT");

        let instrument = Instrument::new(
            ExchangeId::BinanceSpot,
            "binance_spot_btc_usdt",
            "BTC-USDT",
            Underlying::new(base_asset.clone(), quote_asset.clone()),
            InstrumentQuoteAsset::UnderlyingQuote,
            InstrumentKind::Spot,
            Some(InstrumentSpec {
                price: InstrumentSpecPrice {
                    min: dec!(0.1),
                    tick_size: dec!(0.1),
                },
                quantity: InstrumentSpecQuantity {
                    unit: OrderQuantityUnits::Asset(base_asset.clone()),
                    min: dec!(0.001),
                    increment: dec!(0.001),
                },
                notional: InstrumentSpecNotional { min: dec!(10) },
            }),
        );

        let indexed = IndexedInstrumentsBuilder::default()
            .add_instrument(instrument)
            .build();

        // Should index the asset used in OrderQuantityUnits
        assert_eq!(indexed.assets().len(), 2);
        assert_eq!(
            indexed.assets()[0].value,
            exchange_asset(ExchangeId::BinanceSpot, "BTC")
        );
    }

    #[test]
    fn test_builder_ordering() {
        // Add instruments in any order
        let indexed = IndexedInstrumentsBuilder::default()
            .add_instrument(instrument(ExchangeId::BinanceSpot, "BTC", "USDT"))
            .add_instrument(instrument(ExchangeId::Coinbase, "ETH", "USD"))
            .build();

        // Verify exchanges are ordered by input sequence
        assert_eq!(indexed.exchanges()[0].value, ExchangeId::BinanceSpot);
        assert_eq!(indexed.exchanges()[1].value, ExchangeId::Coinbase);

        // Verify exchanges are ordered by input sequence
        assert_eq!(
            indexed.assets()[0].value,
            exchange_asset(ExchangeId::BinanceSpot, "BTC")
        );
        assert_eq!(
            indexed.assets()[1].value,
            exchange_asset(ExchangeId::BinanceSpot, "USDT")
        );
        assert_eq!(
            indexed.assets()[2].value,
            exchange_asset(ExchangeId::Coinbase, "ETH")
        );
        assert_eq!(
            indexed.assets()[3].value,
            exchange_asset(ExchangeId::Coinbase, "USD")
        );

        // Verify instruments are ordered by input sequence
        assert_eq!(
            indexed.instruments()[0].value,
            Instrument {
                exchange: Keyed::new(ExchangeIndex(0), ExchangeId::BinanceSpot),
                name_exchange: InstrumentNameExchange::new("BTC_USDT"),
                name_internal: InstrumentNameInternal::new("binance_spot-btc_usdt"),
                underlying: Underlying {
                    base: AssetIndex(0),
                    quote: AssetIndex(1),
                },
                quote: InstrumentQuoteAsset::UnderlyingQuote,
                kind: InstrumentKind::Spot,
                spec: None
            }
        );

        assert_eq!(
            indexed.instruments()[1].value,
            Instrument {
                exchange: Keyed::new(ExchangeIndex(1), ExchangeId::Coinbase),
                name_exchange: InstrumentNameExchange::new("ETH_USD"),
                name_internal: InstrumentNameInternal::new("coinbase-eth_usd"),
                underlying: Underlying {
                    base: AssetIndex(2),
                    quote: AssetIndex(3),
                },
                quote: InstrumentQuoteAsset::UnderlyingQuote,
                kind: InstrumentKind::Spot,
                spec: None
            }
        );
    }
}
