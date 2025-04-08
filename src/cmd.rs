use crate::epg::{
    BsTv, CsTv, Printer, TodayBsTv, TodayCsTv, TodayTv, Tv, WeekBsTv, WeekCsTv, WeekTv,
};
use anyhow::{anyhow, Result};
use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::LazyLock;
use std::{env, process};
use structopt::{clap, StructOpt};

const ENV_KEY: &str = "TV_AREA";

pub struct Cli<T, U> {
    out_stream: T,
    err_stream: U,
}

impl<T: Write, U: Write> Cli<T, U> {
    pub fn new(out: T, err: U) -> Self {
        Cli {
            out_stream: out,
            err_stream: err,
        }
    }

    pub async fn execute(&mut self, args: impl Iterator<Item = String>) -> ExitCode {
        match self.run(args).await {
            Ok(_) => ExitCode::Normal,
            Err(e) => {
                writeln!(self.err_stream, "{}", e).unwrap();
                ExitCode::Abnormal
            }
        }
    }

    async fn run(&mut self, args: impl Iterator<Item = String>) -> Result<()> {
        // ANSIエスケープコードに基づいて出力を正しく色付けしないWindows 10環境で必要
        #[cfg(target_os = "windows")]
        control::set_virtual_terminal(true).unwrap();
        let opt = self.get_opt(args)?;
        if opt.area {
            return {
                self.print_areas();
                Ok(())
            };
        }
        let default_area = env::var(ENV_KEY).ok();
        let default_area = default_area.as_deref().unwrap_or("tokyo");
        let mut area_id = self.get_area_id(default_area)?;
        if let Some(area_name) = opt.area_name.as_deref() {
            if let Some(&id) = AREA_MAP.get(area_name) {
                area_id = id;
            }
        }
        self.get_tv_printer(area_id, &opt)
            .await?
            .print(&mut self.out_stream);
        Ok(())
    }

    fn get_opt(&self, args: impl Iterator<Item = String>) -> Result<Opt> {
        Ok(Opt::from_iter_safe(args)?)
    }

    fn get_area_id(&self, default: &str) -> Result<u8> {
        AREA_MAP
            .get(default)
            .copied()
            .ok_or_else(|| anyhow!("{} is not in the area", default.bright_yellow()))
    }

    async fn get_tv_printer<W>(&self, area_id: u8, opt: &Opt) -> Result<Box<dyn Printer<W>>>
    where
        W: Write,
    {
        create_printer(area_id, opt).await
    }

    fn print_areas(&mut self) {
        let mut areas = AREA_MAP.iter().map(|(&k, _)| k).collect::<Vec<_>>();
        let mut buf = io::BufWriter::new(&mut self.out_stream);
        areas.sort();
        areas.iter().for_each(|&a| {
            match a {
                "bs" => writeln!(buf, "{}", "bs".bright_yellow()).unwrap(),
                "cs" => writeln!(buf, "{}", "cs".bright_yellow()).unwrap(),
                _ => writeln!(buf, "{}", a).unwrap(),
            };
        });
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "tvnow", about = "tv program display")]
#[structopt(setting(clap::AppSettings::ColoredHelp))]
struct Opt {
    /// Prints today's program
    #[structopt(short, long, conflicts_with_all(&["week", "area"]))]
    today: bool,
    /// Prints a week program
    #[structopt(short, long, conflicts_with_all(&["after", "area"]))]
    week: bool,
    /// Prints area list
    #[structopt(short, long, conflicts_with_all(&["today", "week"]))]
    area: bool,

    #[structopt(name = "AREA", min_values = 0, max_values = 1)]
    area_name: Option<String>,
}

async fn create_printer<T: Write>(area: u8, opt: &Opt) -> Result<Box<dyn Printer<T>>> {
    match area {
        0 if opt.today => TodayBsTv::init().await,
        0 if opt.week => WeekBsTv::init().await,
        0 => BsTv::init().await,
        255 if opt.today => TodayCsTv::init().await,
        255 if opt.week => WeekCsTv::init().await,
        255 => CsTv::init().await,
        i if opt.today => TodayTv::init(i).await,
        i if opt.week => WeekTv::init(i).await,
        i => Tv::init(i).await,
    }
}

#[derive(PartialOrd, PartialEq, Debug, Clone, Copy)]
pub enum ExitCode {
    Normal = 0,
    Abnormal = 1,
}

impl ExitCode {
    pub fn exit(&self) -> ! {
        process::exit(*self as i32)
    }
}

static AREA_MAP: LazyLock<HashMap<&'static str, u8>> = LazyLock::new(|| {
    let m = [
        ("cs", 255),
        ("bs", 0),
        ("sapporo", 1),
        ("hakodate", 8),
        ("asahikawa", 3),
        ("obihiro", 9),
        ("kushiro", 10),
        ("kitami", 12),
        ("muroran", 6),
        ("aomori", 13),
        ("iwate", 16),
        ("miyagi", 19),
        ("akita", 22),
        ("yamagata", 25),
        ("fukushima", 28),
        ("tokyo", 42),
        ("kanagawa", 45),
        ("saitama", 37),
        ("chiba", 40),
        ("ibaragi", 31),
        ("tochigi", 33),
        ("gumma", 35),
        ("yamanashi", 50),
        ("nagano", 51),
        ("niigata", 56),
        ("aichi", 73),
        ("ishikawa", 60),
        ("shizuoka", 67),
        ("fukui", 62),
        ("toyama", 58),
        ("mie", 76),
        ("gifu", 64),
        ("osaka", 84),
        ("kyoto", 81),
        ("hyogo", 85),
        ("wakayama", 93),
        ("nara", 91),
        ("shiga", 79),
        ("hiroshima", 101),
        ("okayama", 98),
        ("shimane", 96),
        ("tottori", 95),
        ("yamaguchi", 105),
        ("ehime", 112),
        ("kagawa", 110),
        ("tokushima", 109),
        ("kochi", 116),
        ("fukuoka", 117),
        ("kumamoto", 126),
        ("nagasaki", 123),
        ("kagoshima", 131),
        ("miyazaki", 129),
        ("oita", 127),
        ("saga", 122),
        ("okinawa", 134),
        ("kitakyushu", 120),
    ]
    .iter()
    .cloned()
    .collect();
    m
});

#[cfg(test)]
mod tests {

    use super::*;
    use colored::control::set_override;

    #[async_std::test]
    async fn test_tv_works() {
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string(), "-a".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);

        let args = vec!["tvnow".to_string(), "osaka".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);
    }
    #[async_std::test]
    async fn test_bs_works() {
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string(), "bs".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);
    }
    #[async_std::test]
    async fn test_today_works() {
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string(), "tokyo".to_string(), "-t".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);

        let args = vec!["tvnow".to_string(), "bs".to_string(), "--today".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);
    }
    #[async_std::test]
    async fn test_week_works() {
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string(), "tokyo".to_string(), "-w".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);

        let args = vec!["tvnow".to_string(), "bs".to_string(), "--week".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);
    }
    #[async_std::test]
    async fn test_flag_error_works() {
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string(), "tokyo".to_string(), "-wt".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Abnormal);

        let args = vec!["tvnow".to_string(), "tokyo".to_string(), "-1".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Abnormal);

        let args = vec![
            "tvnow".to_string(),
            "tokyo".to_string(),
            "osaka".to_string(),
        ];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Abnormal);
    }
    #[async_std::test]
    #[ignore]
    // cargo test -- --ignored --test-threads=1
    async fn test_env_default_area_works() {
        std::env::set_var(ENV_KEY, "hogehoge");
        let mut cli = Cli::new(vec![], vec![]);
        let args = vec!["tvnow".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Abnormal);
        std::env::set_var(ENV_KEY, "tokyo");
    }
    #[async_std::test]
    #[ignore]
    async fn test_env_default_area_error_message_works() {
        // カラー化無効
        set_override(false);
        std::env::set_var(ENV_KEY, "fugafuga");
        let mut out: Vec<u8> = vec![];
        let mut err: Vec<u8> = vec![];
        let mut cli = Cli::new(&mut out, &mut err);
        let args = vec!["tvnow".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Abnormal);
        let err_string = String::from_utf8(err).unwrap();
        assert_eq!(err_string, "fugafuga is not in the area\n");
        std::env::set_var(ENV_KEY, "tokyo");
    }

    #[async_std::test]
    async fn test_tv_tokyo_channel_number_works() {
        //　カラー化無効
        set_override(false);
        let mut out: Vec<u8> = vec![];
        let mut err: Vec<u8> = vec![];
        let mut cli = Cli::new(&mut out, &mut err);

        let args = vec!["tvnow".to_string(), "tokyo".to_string()];
        let result = cli.execute(args.into_iter());
        assert_eq!(result.await, ExitCode::Normal);

        let out_string = String::from_utf8(out).unwrap();
        // 東京のチャンネル番号
        let channels = ["1", "2", "4", "5", "6", "7", "8", "9", "9", "3", "3", "3"];
        for (i, s) in out_string.lines().enumerate() {
            assert!(s.starts_with(channels[i]))
        }
    }
}
