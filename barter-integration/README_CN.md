# Barter-Integration

高性能、底层的灵活 Web 集成框架。

被其他 [`Barter`] 交易生态系统 crate 用于构建稳健的金融交易所集成，主要用于公开数据收集和交易执行。它具有以下特点：
* **底层**：将通过网络传输的原始数据流转换为任意所需的数据模型，使用任意数据转换。
* **灵活**：兼容任何协议（WebSocket、FIX、Http 等）、任何输入/输出模型，以及任何用户定义的转换。

核心抽象包括：
- **RestClient** 提供客户端与服务器之间可配置的签名 Http 通信。
- **ExchangeStream** 提供任何异步流协议（WebSocket、FIX 等）的可配置通信。

两个核心抽象都提供了稳健的粘合剂，帮助您方便地在服务器和客户端数据模型之间进行转换。


**请参阅：[`Barter`]、[`Barter-Data`] 和 [`Barter-Execution`]**

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/barter-integration.svg
[crates-url]: https://crates.io/crates/barter-integration

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/-/blob/main/LICENCE

[actions-badge]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/badges/-/blob/main/pipeline.svg
[actions-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/-/commits/main

[discord-badge]: https://img.shields.io/discord/910237311332151317.svg?logo=discord&style=flat-square
[discord-url]: https://discord.gg/wE7RqhnQMV

[API Documentation] | [Chat]

[`Barter`]: https://crates.io/crates/barter
[`Barter-Data`]: https://crates.io/crates/barter-data
[`Barter-Execution`]: https://crates.io/crates/barter-execution
[API Documentation]: https://docs.rs/barter-data/latest/barter_integration
[Chat]: https://discord.gg/wE7RqhnQMV

## 概述

Barter-Integration 是一个高性能、底层、可配置的灵活 Web 集成框架。

### RestClient
**（同步的私有和公开 Http 通信）**

从高层次来看，`RestClient` 有几个主要组件使其能够执行 `RestRequests`：
* `RequestSigner` 带有针对目标 API 的可配置签名逻辑。
* `HttpParser` 将 API 特定的响应转换为所需的输出类型。

### ExchangeStream
**（使用 WebSocket、FIX 等流协议的异步通信）**

从高层次来看，`ExchangeStream` 由几个主要组件组成：
* 内部 Stream/Sink 套接字（如 WebSocket、FIX 等）。
* StreamParser 能够将输入的协议消息（如 WebSocket、FIX 等）解析为交易所特定的消息。
* Transformer 将交易所特定的消息转换为所需输出类型的迭代器。

## 示例

#### 使用签名 GET 请求获取 Ftx 账户余额：
```rust,no_run
use std::borrow::Cow;

use barter_integration::{
    error::SocketError,
    metric::Tag,
    model::Symbol,
    protocol::http::{
        private::{encoder::HexEncoder, RequestSigner, Signer},
        rest::{client::RestClient, RestRequest},
        HttpParser,
    },
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::{RequestBuilder, StatusCode};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc;

struct FtxSigner {
    api_key: String,
}

// 签署每个 Ftx `RestRequest` 所需的配置
struct FtxSignConfig<'a> {
    api_key: &'a str,
    time: DateTime<Utc>,
    method: reqwest::Method,
    path: Cow<'static, str>,
}

impl Signer for FtxSigner {
    type Config<'a> = FtxSignConfig<'a> where Self: 'a;

    fn config<'a, Request>(
        &'a self,
        request: Request,
        _: &RequestBuilder,
    ) -> Result<Self::Config<'a>, SocketError>
    where
        Request: RestRequest,
    {
        Ok(FtxSignConfig {
            api_key: self.api_key.as_str(),
            time: Utc::now(),
            method: Request::method(),
            path: request.path(),
        })
    }

    fn add_bytes_to_sign<M>(mac: &mut M, config: &Self::Config<'a>) -> Bytes
    where
        M: Mac
    {
        mac.update(config.time.to_string().as_bytes());
        mac.update(config.method.as_str().as_bytes());
        mac.update(config.path.as_bytes());
    }

    fn build_signed_request<'a>(
        config: Self::Config<'a>,
        builder: RequestBuilder,
        signature: String,
    ) -> Result<reqwest::Request, SocketError> {
        // 添加 Ftx 所需的 Headers 并构建 reqwest::Request
        builder
            .header("FTX-KEY", config.api_key)
            .header("FTX-TS", &config.time.timestamp_millis().to_string())
            .header("FTX-SIGN", &signature)
            .build()
            .map_err(SocketError::from)
    }
}

struct FtxParser;

impl HttpParser for FtxParser {
    type ApiError = serde_json::Value;
    type OutputError = ExecutionError;

    fn parse_api_error(&self, status: StatusCode, api_error: Self::ApiError) -> Self::OutputError {
        // 为简单起见，使用 serde_json::Value 作为错误并提取原始字符串进行解析
        let error = api_error.to_string();

        // 解析 Ftx 错误消息以确定自定义 ExecutionError 变体
        match error.as_str() {
            message if message.contains("Invalid login credentials") => {
                ExecutionError::Unauthorised(error)
            }
            _ => ExecutionError::Socket(SocketError::HttpResponse(status, error)),
        }
    }
}

#[derive(Debug, Error)]
enum ExecutionError {
    #[error("request authorisation invalid: {0}")]
    Unauthorised(String),

    #[error("SocketError: {0}")]
    Socket(#[from] SocketError),
}

struct FetchBalancesRequest;

impl RestRequest for FetchBalancesRequest {
    type Response = FetchBalancesResponse; // 定义响应类型
    type QueryParams = (); // FetchBalances 不需要任何 QueryParams
    type Body = (); // FetchBalances 不需要任何 Body

    fn path(&self) -> Cow<'static, str> {
        Cow::Borrowed("/api/wallet/balances")
    }

    fn method() -> reqwest::Method {
        reqwest::Method::GET
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FetchBalancesResponse {
    success: bool,
    result: Vec<FtxBalance>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FtxBalance {
    #[serde(rename = "coin")]
    symbol: Symbol,
    total: f64,
}

/// 请参阅 Barter-Execution 获取完整的实际示例，以及可以开箱即用的代码
/// 来在多个交易所执行交易。
#[tokio::main]
async fn main() {
    // HMAC-SHA256 编码的账户 API 密钥，用于签署私有 http 请求
    let mac: Hmac<sha2::Sha256> = Hmac::new_from_slice("api_secret".as_bytes()).unwrap();

    // 构建配置了十六进制编码的 Ftx RequestSigner 用于签署 http 请求
    let request_signer = RequestSigner::new(
        FtxSigner {
            api_key: "api_key".to_string(),
        },
        mac,
        HexEncoder,
    );

    // 使用 Ftx 配置构建 RestClient
    let rest_client = RestClient::new("https://ftx.com", request_signer, FtxParser);

    // 获取 Result<FetchBalancesResponse, ExecutionError>
    let _response = rest_client.execute(FetchBalancesRequest).await;
}
```

#### 消费 Binance Futures 逐笔成交数据并计算成交量滚动求和：

```rust,no_run
use barter_integration::{
    error::SocketError,
    protocol::websocket::{WebSocket, WebSocketSerdeParser, WsMessage},
    ExchangeStream, Transformer,
};
use futures::{SinkExt, StreamExt};
use serde::{de, Deserialize};
use serde_json::json;
use std::str::FromStr;
use tokio_tungstenite::connect_async;
use tracing::debug;

// 使用 tungstenite `WebSocket` 的 `ExchangeStream` 类型别名
type ExchangeWsStream<Exchange> = ExchangeStream<WebSocketSerdeParser, WebSocket, Exchange, VolumeSum>;

// VolumeSum 的通信性类型别名，表示 Transformer 正在生成的内容
type VolumeSum = f64;

#[derive(Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
enum BinanceMessage {
    SubResponse {
        result: Option<Vec<String>>,
        id: u32,
    },
    Trade {
        #[serde(rename = "q", deserialize_with = "de_str")]
        quantity: f64,
    },
}

struct StatefulTransformer {
    sum_of_volume: VolumeSum,
}

impl Transformer<VolumeSum> for StatefulTransformer {
    type Input = BinanceMessage;
    type OutputIter = Vec<Result<VolumeSum, SocketError>>;

    fn transform(&mut self, input: Self::Input) -> Self::OutputIter {
        // 将新输入的 Trade 数量添加到求和中
        match input {
            BinanceMessage::SubResponse { result, id } => {
                debug!("Received SubResponse for {}: {:?}", id, result);
                // 对于此示例我们不关心这个
            }
            BinanceMessage::Trade { quantity, .. } => {
                // 将新的 Trade 成交量添加到内部状态 VolumeSum
                self.sum_of_volume += quantity;
            }
        };

        // 返回长度为 1 的 IntoIterator，包含成交量的运行求和
        vec![Ok(self.sum_of_volume)]
    }
}

/// 请参阅 Barter-Data 获取完整的实际示例，以及可以开箱即用的代码
/// 来从多个交易所收集实时公开市场数据。
#[tokio::main]
async fn main() {
    // 与所需的 WebSocket 服务器建立 Sink/Stream 通信
    let mut binance_conn = connect_async("wss://fstream.binance.com/ws/")
        .await
        .map(|(ws_conn, _)| ws_conn)
        .expect("failed to connect");

    // 通过套接字发送内容（例如 Binance trades 订阅）
    binance_conn
        .send(WsMessage::Text(
            json!({"method": "SUBSCRIBE","params": ["btcusdt@aggTrade"],"id": 1}).to_string(),
        ))
        .await
        .expect("failed to send WsMessage over socket");

    // 实例化一个任意的 Transformer 来应用于从 WebSocket 协议解析的数据
    let transformer = StatefulTransformer { sum_of_volume: 0.0 };

    // ExchangeWsStream 包含预定义的 WebSocket Sink/Stream 和 WebSocket StreamParser
    let mut ws_stream = ExchangeWsStream::new(binance_conn, transformer);

    // 从 ExchangeStream 接收所需输出数据模型的流
    while let Some(volume_result) = ws_stream.next().await {
        match volume_result {
            Ok(cumulative_volume) => {
                // 对您的数据进行处理
                println!("{cumulative_volume:?}");
            }
            Err(error) => {
                // 对内部转换产生的任何错误做出反应
                eprintln!("{error}")
            }
        }
    }
}

/// 将 `String` 反序列化为所需类型。
fn de_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: de::Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let data: String = Deserialize::deserialize(deserializer)?;
    data.parse::<T>().map_err(de::Error::custom)
}
```

#### 解析二进制 protobuf 消息

`WebSocketProtobufParser` 可以使用 [`prost`] 解码 `WsMessage::Binary` 负载。当服务器发送 protobuf 编码的消息时，它可以替代 `WebSocketSerdeParser` 与 `ExchangeStream` 一起使用。

```rust
use barter_integration::protocol::websocket::{WebSocket, WebSocketProtobufParser};
use barter_integration::ExchangeStream;

type ProtoStream<Exchange> = ExchangeStream<WebSocketProtobufParser, WebSocket, Exchange, ()>;
```

[`prost`]: https://crates.io/crates/prost

**如需更大型的"真实世界"示例，请参阅 [`Barter-Data`] 仓库。**

## 获取帮助
首先，请查看 [API 文档][API Documentation] 中是否有您问题的答案。如果没有找到，我很乐意通过 [Discord 聊天][Chat] 来帮助您解答问题。

## 贡献
感谢您帮助改进 Barter 生态系统！如有开发、新功能和未来路线图方面的讨论，请随时通过 Discord 联系我们。

## 相关项目
除了 Barter-Integration crate 之外，Barter 项目还维护：
* [`Barter`]：高性能、可扩展且模块化的交易组件，开箱即用。包含预构建的交易引擎，可用作实盘交易或回测系统。
* [`Barter-Data`]：高性能的 WebSocket 集成库，用于从领先的加密货币交易所流式获取公开数据。
* [`Barter-Execution`]：用于交易执行的金融交易所集成 - 尚未发布！

## 路线图
* 添加新的默认 StreamParser 实现，以支持与其他流行系统（如 Kafka）的集成。

## 许可证
本项目采用 [MIT 许可证]授权。

[MIT 许可证]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-data-rs/-/blob/main/LICENSE

### 贡献
除非您另有明确说明，否则您有意提交的任何用于包含在 Barter-Integration 中的贡献应采用 MIT 许可证授权，不附带任何额外条款或条件。
