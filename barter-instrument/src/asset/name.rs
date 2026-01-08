use derive_more::Display;
use serde::Serialize;
use smol_str::{SmolStr, StrExt};
use std::borrow::Borrow;

/// Barter lowercase `SmolStr` representation for an [`Asset`](super::Asset) - not unique across
/// exchanges.
///
/// This may or may not be different from an execution's representation.
///
/// For example, some exchanges may refer to "btc" as "xbt".
/// molStr (神奇之处)：
///
/// 它是一个栈上优先的字符串。
///
/// 如果字符串长度不超过 23 字节（绝大多数加密货币代码如 "BTC", "ETH-USDT" 都很短），它直接存放在结构体内部（Inline），完全没有堆内存分配。
///
/// 它的性能接近于 C 语言的 char[23] 数组。
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Display)]
pub struct AssetNameInternal(SmolStr);

impl AssetNameInternal {
    /// Construct a new lowercase [`Self`] from the provided `Into<SmolStr>`.
    pub fn new<S>(name: S) -> Self
    where
        S: Into<SmolStr>,
    {
        let name = name.into();

        //判断字符串 name 中的每一个字符是否都是小写字母
        if name.chars().all(char::is_lowercase) {
            Self(name)
        } else {
            Self(name.to_lowercase_smolstr())
        }
    }

    /// Return the internal asset `SmolStr` name of [`Self`].
    pub fn name(&self) -> &SmolStr {
        &self.0
    }
}

impl From<&str> for AssetNameInternal {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for AssetNameInternal {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

impl From<String> for AssetNameInternal {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Borrow<str> for AssetNameInternal {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl AsRef<str> for AssetNameInternal {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'de> serde::de::Deserialize<'de> for AssetNameInternal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        let name =
            <std::borrow::Cow<'de, str> as serde::Deserialize<'de>>::deserialize(deserializer)?;
        Ok(AssetNameInternal::new(name))
    }
}

/// Exchange `SmolStr` representation for an [`Asset`](super::Asset) - not unique across exchanges.
///
/// For example: `AssetNameExchange("XBT")`, which is distinct from the internal representation
/// of the asset, such as `AssetIndex(1)` or `AssetNameInternal("btc")`.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Display)]
pub struct AssetNameExchange(SmolStr);

impl AssetNameExchange {
    /// Construct a new [`Self`] from the provided `Into<SmolStr>`.
    pub fn new<S>(name: S) -> Self
    where
        S: Into<SmolStr>,
    {
        Self(name.into())
    }

    /// Return the execution asset `SmolStr` name of [`Self`].
    pub fn name(&self) -> &SmolStr {
        &self.0
    }
}

impl From<&str> for AssetNameExchange {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<SmolStr> for AssetNameExchange {
    fn from(value: SmolStr) -> Self {
        Self::new(value)
    }
}

impl From<String> for AssetNameExchange {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl Borrow<str> for AssetNameExchange {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl AsRef<str> for AssetNameExchange {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'de> serde::de::Deserialize<'de> for AssetNameExchange {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(AssetNameExchange::new(name))
    }
}

#[cfg(test)]
#[allow(unused_variables)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    #[test]
    fn test_asset_name_internal() {
        let orderbook: HashMap<AssetNameInternal, f64> = HashMap::new();

        // 这太蠢了！ 我只是想查个表，为什么要创建一个全新的对象？对于高频交易系统，这多出来的一次构造开销是不可接受的
        orderbook.get(&AssetNameInternal::new("BTC"));

        // 这里是由borrow 这个 trait 保证的
        orderbook.get("btc");

        let eth_name = AssetNameInternal::new("ETH");
        let borrow_eth_name: &str = eth_name.borrow();

        let borrow1 = <AssetNameInternal as Borrow<str>>::borrow(&eth_name);

        println!("{}", borrow_eth_name);
    }

    // 这个函数说：我不强制你要 AssetNameInternal，
    // 只要是你传进来的东西(S)，能自动变成(Into) AssetNameInternal 就行！
    fn process_trade<S>(asset: S)
    where
        S: Into<AssetNameInternal>,
    {
        let real_asset: AssetNameInternal = asset.into();
        println!("Processing: {}", real_asset);
    }
    #[test]
    fn test_from_into() {
        // 这里的 "BTC" 会进入 from 方法 -> 调用 new 方法 -> 自动转为小写 "btc"
        let asset = AssetNameInternal::from("BTC");

        println!("{:?}", asset); // 输出 AssetNameInternal("btc")

        // 编译器看左边：哦，你要 AssetNameInternal
        // 编译器看右边：是一个 &str
        // 编译器：我知道 &str 实现了 Into<AssetNameInternal>，自动转换！
        let asset: AssetNameInternal = "BTC".into();
        // < 类型 as 接口 > :: 函数名
        // < Type as Trait > :: method_name
        {
            let asset = AssetNameInternal::from("BTC");

            let another_asset = <AssetNameInternal as From<&str>>::from("BTC");
        }

        // 爽点来了！直接传字符串字面量！
        // 编译器会自动调用你写的 From<&str> 逻辑
        process_trade("BTC");

        // 传 String 也可以（前提是你也实现了 From<String>）
        process_trade("ETH".to_string());
    }
}
