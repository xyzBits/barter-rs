use crate::{asset::name::AssetNameExchange, exchange::ExchangeId};
use derive_more::Display;
use serde::Serialize;
use smol_str::{SmolStr, StrExt, format_smolstr};
use std::borrow::Borrow;

/// Barter 系统内部使用的交易工具名称（小写）。
/// 在整个系统中，即使交易所、基础资产、计价资产都相同，通常也会加上交易所前缀以确保全局唯一性。
/// 示例：`InstrumentNameInternal("binance_spot-btc_usdt")`
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Display)]
pub struct InstrumentNameInternal(pub SmolStr);
impl InstrumentNameInternal {
    /// 从任何可转换为 `SmolStr` 的类型（如 `&str`, `String`）创建一个新的小写名称。
    /// ### 泛型参数：
    /// * `S`: 输入名称类型。
    /// ### 示例：
    /// `InstrumentNameInternal::new("Binance_Spot-BTC_USDT")` -> 存储为 "binance_spot-btc_usdt"。
    pub fn new<S>(name: S) -> Self
    where
        S: Into<SmolStr>,
    {
        let name = name.into();
        if name.chars().all(char::is_lowercase) {
            Self(name)
        } else {
            Self(name.to_lowercase_smolstr())
        }
    }

    /// 根据交易所 ID、基础资产和计价资产的官方名称构造一个内部唯一的标识符。
    /// ### 参数：
    /// * `exchange`: 交易所 ID。
    /// * `base`: 基础资产的官方名称（如 "BTC"）。
    /// * `quote`: 计价资产的官方名称（如 "USDT"）。
    pub fn new_from_exchange_underlying<Ass>(exchange: ExchangeId, base: &Ass, quote: &Ass) -> Self
    where
        for<'a> &'a Ass: Into<&'a AssetNameExchange>,
    {
        Self::new(format_smolstr!(
            "{exchange}-{}_{}",
            base.into(),
            quote.into()
        ))
    }

    /// Construct a new lowercase [`Self`], combining the [`ExchangeId`] and
    /// [`InstrumentNameExchange`].
    ///
    /// Generates an internal instrument identifier unique across exchanges.
    pub fn new_from_exchange<S>(exchange: ExchangeId, name_exchange: S) -> Self
    where
        S: Into<InstrumentNameExchange>,
    {
        let name_exchange = name_exchange.into();
        let exchange = exchange.as_str();
        Self::new(format_smolstr!("{exchange}-{name_exchange}"))
    }

    /// Return the internal instrument `SmolStr` name of [`Self`].
    pub fn name(&self) -> &SmolStr {
        &self.0
    }
}

impl From<&str> for InstrumentNameInternal {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for InstrumentNameInternal {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

impl From<String> for InstrumentNameInternal {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Borrow<str> for InstrumentNameInternal {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl AsRef<str> for InstrumentNameInternal {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'de> serde::de::Deserialize<'de> for InstrumentNameInternal {
    /// `'de` 是反序列化生命周期，表示输入数据的存续期。
    /// ### 泛型参数：
    /// * `D`: 反序列化器（如 JSON 解释器）。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // Cow 表示 "Copy on Write"。它允许我们在不需要修改数据时引用原始 JSON 字符串，
        // 从而减少不必要的内存拷贝，提高性能。
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(InstrumentNameInternal::new(name))
    }
}

/// 交易所官方使用的产品标识符。
/// ### 示例：
/// `InstrumentNameExchange("XBTUSDT")` (Binance) 或 `InstrumentNameExchange("tBTCUSD")` (Bitfinex)。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Display)]
pub struct InstrumentNameExchange(SmolStr);

impl InstrumentNameExchange {
    /// Construct a new [`Self`] from the provided `Into<SmolStr>`.
    pub fn new<S>(name: S) -> Self
    where
        S: Into<SmolStr>,
    {
        Self(name.into())
    }

    /// Return the execution instrument `SmolStr` name of [`Self`].
    pub fn name(&self) -> &SmolStr {
        &self.0
    }
}

impl From<&str> for InstrumentNameExchange {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for InstrumentNameExchange {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

impl From<String> for InstrumentNameExchange {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Borrow<str> for InstrumentNameExchange {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl AsRef<str> for InstrumentNameExchange {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'de> serde::de::Deserialize<'de> for InstrumentNameExchange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(InstrumentNameExchange::new(name))
    }
}
