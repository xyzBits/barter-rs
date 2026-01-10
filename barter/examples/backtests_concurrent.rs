//! # 并行回测示例 (Concurrent Backtests)
//!
//! 该示例演示了如何使用 `run_backtests` 函数高效地并行运行大规模回测。
//! 通过将回测参数拆分为常量部分和动态部分，可以在利用多核性能的同时，灵活地进行参数搜索（如策略优化）。

use barter::{
    backtest::{
        BacktestArgsConstant, BacktestArgsDynamic,
        market_data::{BacktestMarketData, MarketDataInMemory},
        run_backtests,
    },
    engine::state::{
        EngineState, builder::EngineStateBuilder, global::DefaultGlobalData,
        instrument::data::DefaultInstrumentMarketData, trading::TradingState,
    },
    risk::DefaultRiskManager,
    statistic::time::Daily,
    strategy::DefaultStrategy,
    system::config::SystemConfig,
};
use barter_data::streams::consumer::MarketStreamEvent;
use barter_instrument::index::IndexedInstruments;
use rust_decimal::Decimal;
use serde::Deserialize;
use smol_str::{SmolStr, ToSmolStr};
use std::{
    fs::File,
    io::{BufRead, BufReader},
    sync::Arc,
};

const CONFIG_PATH: &str = "barter/examples/config/backtest_config.json";
const FILE_PATH_MARKET_DATA_INDEXED: &str =
    "barter/examples/data/binance_spot_trades_l1_btcusdt_ethusdt_solusdt.json";
const NUM_BACKTESTS: usize = 10000;

#[derive(Deserialize)]
pub struct Config {
    pub risk_free_return: Decimal,
    pub system: SystemConfig,
}

#[tokio::main]
async fn main() {
    // 1. 初始化日志追踪
    barter::logging::init_logging();

    // 2. 加载配置
    let Config {
        risk_free_return,
        system: SystemConfig {
            instruments,
            executions,
        },
    } = load_config();

    // 3. 构建索引化的交易工具 (IndexedInstruments)
    let instruments = IndexedInstruments::new(instruments);

    // 4. 初始化市场数据
    let market_events = market_data_from_file(FILE_PATH_MARKET_DATA_INDEXED);
    let market_data = MarketDataInMemory::new(Arc::new(market_events));
    let time_engine_start = market_data.time_first_event().await.unwrap();

    // 5. 构建引擎状态 (EngineState)
    let engine_state = EngineStateBuilder::new(&instruments, DefaultGlobalData::default(), |_| {
        DefaultInstrumentMarketData::default()
    })
    .time_engine_start(time_engine_start)
    .trading_state(TradingState::Enabled)
    .build();

    // 6. 构造常量回测参数 (BacktestArgsConstant)
    // 这些参数在所有并行回测任务中是共用的。
    let args_constant = Arc::new(BacktestArgsConstant {
        instruments,
        executions,
        market_data,
        summary_interval: Daily,
        engine_state,
    });

    // 7. 定义动态回测参数模板 (BacktestArgsDynamic)
    let dynamic_arg = BacktestArgsDynamic {
        id: SmolStr::default(),
        risk_free_return,
        strategy: DefaultStrategy::<EngineState<DefaultGlobalData, DefaultInstrumentMarketData>>::default(),
        risk: DefaultRiskManager::<EngineState<DefaultGlobalData, DefaultInstrumentMarketData>>::default(),
    };

    // 8. 生成动态参数迭代器
    // 注意：并行回测应使用不同的 BacktestArgsDynamic（如不同的策略参数或 ID）！
    let args_dynamic_iter = (0..NUM_BACKTESTS).map(|index| {
        let mut dynamic_args = dynamic_arg.clone();
        dynamic_args.id = index.to_smolstr();
        dynamic_args
    });

    // 9. 执行并行回测
    let mut summary = run_backtests(args_constant, args_dynamic_iter)
        .await
        .unwrap();

    // 10. 分析回测汇总结果...
    println!("\n回测数量: {}", summary.num_backtests);
    println!("耗时: {:?}", summary.duration);
    // 例如：找出累计盈亏最高的测试结果。
    summary.summaries.sort_by(|a, b| {
        let backtest_a_total_pnl = a
            .trading_summary
            .instruments
            .values()
            .map(|tear| tear.pnl)
            .sum::<Decimal>();
        let backtest_b_total_pnl = b
            .trading_summary
            .instruments
            .values()
            .map(|tear| tear.pnl)
            .sum::<Decimal>();

        backtest_a_total_pnl.cmp(&backtest_b_total_pnl).reverse()
    });
    let best_cumulative_sharpe = summary.summaries.first().unwrap();

    println!(
        "\n最佳累计收益结果: BacktestId = {}",
        best_cumulative_sharpe.id
    );
    best_cumulative_sharpe.trading_summary.print_summary()
}

pub fn load_config() -> Config {
    let file = File::open(CONFIG_PATH).expect("Failed to open config file");
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).expect("Failed to parse config file")
}

pub fn market_data_from_file<InstrumentKey, Kind>(
    file_path: &str,
) -> Vec<MarketStreamEvent<InstrumentKey, Kind>>
where
    InstrumentKey: for<'de> Deserialize<'de>,
    Kind: for<'de> Deserialize<'de>,
{
    let file = File::open(file_path).unwrap();
    let reader = BufReader::new(file);

    reader
        .lines()
        .map(|line_result| {
            let line = line_result.unwrap();
            serde_json::from_str::<MarketStreamEvent<InstrumentKey, Kind>>(&line).unwrap()
        })
        .collect()
}
