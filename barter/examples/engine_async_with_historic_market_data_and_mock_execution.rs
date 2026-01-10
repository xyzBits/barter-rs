//! # 异步引擎历史数据回测示例 (Async Engine with Historic Data)
//!
//! 该示例演示了如何使用 `SystemBuilder` 设置一个异步引擎，并使用由历史市场数据事件组成的异步流进行驱动。
//! 它涵盖了系统的初始化、异步反馈流的处理、以及如何安全地关闭系统。

use barter::{
    EngineEvent,
    engine::{
        clock::HistoricalClock,
        state::{
            global::DefaultGlobalData,
            instrument::{data::DefaultInstrumentMarketData, filter::InstrumentFilter},
            trading::TradingState,
        },
    },
    logging::init_logging,
    risk::DefaultRiskManager,
    statistic::time::Daily,
    strategy::DefaultStrategy,
    system::{
        builder::{AuditMode, EngineFeedMode, SystemArgs, SystemBuilder},
        config::SystemConfig,
    },
};
use barter_data::{
    event::DataKind,
    streams::{
        consumer::{MarketStreamEvent, MarketStreamResult},
        reconnect::{Event, stream::ReconnectingStream},
    },
};
use barter_instrument::{index::IndexedInstruments, instrument::InstrumentIndex};
use futures::{Stream, StreamExt, stream};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{fs::File, io::BufReader, time::Duration};
use tracing::{info, warn};

const FILE_PATH_SYSTEM_CONFIG: &str = "barter/examples/config/system_config.json";
const FILE_PATH_HISTORIC_MARKET_EVENTS: &str =
    "barter/examples/data/binance_spot_market_data_with_disconnect_events.json";
const RISK_FREE_RETURN: Decimal = dec!(0.05);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 初始化日志追踪
    init_logging();

    // 2. 加载系统配置
    let SystemConfig {
        instruments,
        executions,
    } = load_config()?;

    // 3. 构建索引化的交易工具 (IndexedInstruments)
    let instruments = IndexedInstruments::new(instruments);

    // 4. 初始化历史时钟 (HistoricalClock) 和市场数据流 (MarketStream)
    let (clock, market_stream) =
        init_historic_clock_and_market_stream(FILE_PATH_HISTORIC_MARKET_EVENTS);

    // 5. 构造系统参数 (SystemArgs)
    let args = SystemArgs::new(
        &instruments,
        executions,
        clock,
        DefaultStrategy::default(),
        DefaultRiskManager::default(),
        market_stream,
        DefaultGlobalData::default(),
        |_| DefaultInstrumentMarketData::default(),
    );

    // 6. 使用 SystemBuilder 构建并运行系统：
    // 关于所有配置选项，详见 SystemBuilder 文档。
    let system = SystemBuilder::new(args)
        // 引擎驱动模式：异步 Stream 模式
        .engine_feed_mode(EngineFeedMode::Stream)
        // 禁用审计流（引擎不会发送审计事件）
        .audit_mode(AuditMode::Disabled)
        // 初始交易状态：启用
        .trading_state(TradingState::Enabled)
        // 构建系统，但尚未生成具体任务
        .build::<EngineEvent, _>()?
        // 初始化系统，在当前运行时上派生各组件任务
        .init_with_runtime(tokio::runtime::Handle::current())
        .await?;

    // 7. 让示例运行 5 秒...
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 8. 关机前清理：取消所有订单并平掉所有仓位
    system.cancel_orders(InstrumentFilter::None);
    system.close_positions(InstrumentFilter::None);

    // 9. 系统关机
    let (engine, _shutdown_audit) = system.shutdown().await?;

    // 10. 生成日度交易汇总报告 (TradingSummary<Daily>)
    let trading_summary = engine
        .trading_summary_generator(RISK_FREE_RETURN)
        .generate(Daily);

    // 11. 打印报告
    trading_summary.print_summary();

    Ok(())
}

fn load_config() -> Result<SystemConfig, Box<dyn std::error::Error>> {
    let file = File::open(FILE_PATH_SYSTEM_CONFIG)?;
    let reader = BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}

// Note that there are far more intelligent ways of streaming historical market data, this is
// just for demonstration purposes.
//
// For example:
// - Stream from database
// - Stream from file (more efficiently)
fn init_historic_clock_and_market_stream(
    file_path: &str,
) -> (
    HistoricalClock,
    impl Stream<Item = MarketStreamEvent<InstrumentIndex, DataKind>> + use<>,
) {
    let data = std::fs::read_to_string(file_path).unwrap();
    let events =
        serde_json::from_str::<Vec<MarketStreamResult<InstrumentIndex, DataKind>>>(&data).unwrap();

    let time_exchange_first = events
        .iter()
        .find_map(|result| match result {
            MarketStreamResult::Item(Ok(event)) => Some(event.time_exchange),
            _ => None,
        })
        .unwrap();

    let clock = HistoricalClock::new(time_exchange_first);

    let stream = stream::iter(events)
        .with_error_handler(|error| warn!(?error, "MarketStream generated error"))
        .inspect(|event| match event {
            Event::Reconnecting(exchange) => {
                info!(%exchange, "sending historical disconnection to Engine")
            }
            Event::Item(event) => {
                info!(
                    exchange = %event.exchange,
                    instrument = %event.instrument,
                    kind = event.kind.kind_name(),
                    "sending historical event to Engine"
                )
            }
        });

    (clock, stream)
}
