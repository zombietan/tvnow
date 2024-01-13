use anyhow::{anyhow, Context, Result};
use async_std::task;
use chrono::prelude::*;
use chrono::Duration;
use colored::{Color, Colorize};
use htmlize::unescape;
use scraper::{Html, Selector};
use std::io::{self, Write};
use std::time::Instant;
use surf::{Client, Config};

const TV_GUIDE_START_TIME: u32 = 5;
const TVCOLOR: Color = Color::BrightYellow;
const BSCOLOR: Color = Color::BrightCyan;
const CSCOLOR: Color = Color::TrueColor {
    r: 255,
    g: 165,
    b: 0,
};
const HTTP_KEEP_ALIVE: bool = false;

pub trait Printer<T: Write> {
    fn print(&self, w: T);
}

pub struct Tv {
    epg_doc: Html,
}

impl Tv {
    pub async fn init<T: Write>(id: u8) -> Result<Box<dyn Printer<T>>> {
        let url = format!("https://bangumi.org/epg/td?ggm_group_id={}", id);
        let html = get_html(&url).await?;
        let printer = Box::new(Tv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for Tv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let current_selector = Selector::parse("li.sc-current").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            match ul.select(&current_selector).next() {
                Some(current) => {
                    if let Some(title) = current.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {}",
                            channels[i].color(TVCOLOR),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
                None => writeln!(buf, "{} 現在放送していません", channels[i]).unwrap(),
            }
        }
    }
}

pub struct TodayTv {
    epg_doc: Html,
}

impl TodayTv {
    pub async fn init<T: Write>(id: u8) -> Result<Box<dyn Printer<T>>> {
        let url = format!("https://bangumi.org/epg/td?ggm_group_id={}", id);
        let html = get_html(&url).await?;
        let printer = Box::new(TodayTv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for TodayTv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let future_selector = Selector::parse("li.sc-future").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            writeln!(buf, "{}", channels[i].color(TVCOLOR)).unwrap();
            for li in ul.select(&future_selector) {
                let start = li.value().attr("s").unwrap();
                let start_hours = start.get(8..10).unwrap();
                let start_minutes = start.get(10..12).unwrap();
                let end = li.value().attr("e").unwrap();
                let end_hours = end.get(8..10).unwrap();
                let end_minutes = end.get(10..12).unwrap();
                if let Some(title) = li.select(&title_selector).next() {
                    writeln!(
                        buf,
                        "{}:{} ~ {}:{} {}",
                        start_hours,
                        start_minutes,
                        end_hours,
                        end_minutes,
                        unescape(title.inner_html())
                    )
                    .unwrap();
                }
            }
        }
    }
}

pub struct WeekTv {
    epg_docs: Vec<Html>,
}

impl WeekTv {
    pub async fn init<T: Write>(id: u8) -> Result<Box<dyn Printer<T>>> {
        let mut datetime = Local::now();
        if datetime.hour() < TV_GUIDE_START_TIME {
            datetime = Local::now() + Duration::days(-1);
        }
        const WEEK_COUNT: usize = 8;
        let mut urls: [String; WEEK_COUNT] = Default::default();
        for index in urls.iter_mut().take(WEEK_COUNT) {
            let today = datetime.format("%Y%m%d");
            let url = format!(
                "https://bangumi.org/epg/td?broad_cast_date={}&ggm_group_id={}",
                today, id
            );
            *index = url;
            datetime += Duration::days(1);
        }
        let htmls = async_get_htmls(urls.to_vec()).await?;
        let printer = Box::new(WeekTv { epg_docs: htmls });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for WeekTv {
    fn print(&self, w: T) {
        let mut buf = io::BufWriter::new(w);
        for epg_doc in &self.epg_docs {
            let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
            let channels = epg_doc
                .select(&ch_selector)
                .map(|e| e.inner_html())
                .collect::<Vec<_>>();
            let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

            let program_selector = Selector::parse("div#program_area ul").unwrap();
            let future_selector = Selector::parse("li.sc-future").unwrap();
            let title_selector = Selector::parse("p.program_title").unwrap();

            let program_area = epg_doc.select(&program_selector);
            for (i, ul) in program_area.enumerate() {
                for li in ul.select(&future_selector) {
                    let start = li.value().attr("s").unwrap();
                    let end = li.value().attr("e").unwrap();
                    let start = NaiveDateTime::parse_from_str(start, "%Y%m%d%H%M").unwrap();
                    let end = NaiveDateTime::parse_from_str(end, "%Y%m%d%H%M").unwrap();
                    if let Some(title) = li.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {} ~ {} {}",
                            channels[i],
                            start.format("%a %R"),
                            end.format("%a %R"),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
}

pub struct BsTv {
    epg_doc: Html,
}

impl BsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let url = "https://bangumi.org/epg/bs";
        let html = get_html(url).await?;

        let printer = Box::new(BsTv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for BsTv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let current_selector = Selector::parse("li.sc-current").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            match ul.select(&current_selector).next() {
                Some(current) => {
                    if let Some(title) = current.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {}",
                            channels[i].color(BSCOLOR),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
                None => writeln!(buf, "{} 現在放送していません", channels[i]).unwrap(),
            }
        }
    }
}

pub struct TodayBsTv {
    epg_doc: Html,
}

impl TodayBsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let url = "https://bangumi.org/epg/bs";
        let html = get_html(url).await?;

        let printer = Box::new(TodayBsTv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for TodayBsTv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let future_selector = Selector::parse("li.sc-future").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            writeln!(buf, "{}", channels[i].color(BSCOLOR)).unwrap();
            for li in ul.select(&future_selector) {
                let start = li.value().attr("s").unwrap();
                let start_hours = start.get(8..10).unwrap();
                let start_minutes = start.get(10..12).unwrap();
                let end = li.value().attr("e").unwrap();
                let end_hours = end.get(8..10).unwrap();
                let end_minutes = end.get(10..12).unwrap();
                if let Some(title) = li.select(&title_selector).next() {
                    writeln!(
                        buf,
                        "{}:{} ~ {}:{} {}",
                        start_hours,
                        start_minutes,
                        end_hours,
                        end_minutes,
                        unescape(title.inner_html())
                    )
                    .unwrap();
                }
            }
        }
    }
}

pub struct WeekBsTv {
    epg_docs: Vec<Html>,
}

impl WeekBsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let mut datetime = Local::now();
        if datetime.hour() < TV_GUIDE_START_TIME {
            datetime = Local::now() + Duration::days(-1);
        }
        const WEEK_COUNT: usize = 8;
        let mut urls: [String; WEEK_COUNT] = Default::default();
        for index in urls.iter_mut().take(WEEK_COUNT) {
            let today = datetime.format("%Y%m%d");
            let url = format!("https://bangumi.org/epg/bs?broad_cast_date={}", today);
            *index = url;
            datetime += Duration::days(1);
        }
        let htmls = async_get_htmls(urls.to_vec()).await?;
        let printer = Box::new(WeekBsTv { epg_docs: htmls });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for WeekBsTv {
    fn print(&self, w: T) {
        let mut buf = io::BufWriter::new(w);
        for epg_doc in &self.epg_docs {
            let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
            let channels = epg_doc
                .select(&ch_selector)
                .map(|e| e.inner_html())
                .collect::<Vec<_>>();
            let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

            let program_selector = Selector::parse("div#program_area ul").unwrap();
            let future_selector = Selector::parse("li.sc-future").unwrap();
            let title_selector = Selector::parse("p.program_title").unwrap();

            let program_area = epg_doc.select(&program_selector);
            for (i, ul) in program_area.enumerate() {
                for li in ul.select(&future_selector) {
                    let start = li.value().attr("s").unwrap();
                    let end = li.value().attr("e").unwrap();
                    let start = NaiveDateTime::parse_from_str(start, "%Y%m%d%H%M").unwrap();
                    let end = NaiveDateTime::parse_from_str(end, "%Y%m%d%H%M").unwrap();
                    if let Some(title) = li.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {} ~ {} {}",
                            channels[i],
                            start.format("%a %R"),
                            end.format("%a %R"),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
}

pub struct CsTv {
    epg_doc: Html,
}

impl CsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let url = "https://bangumi.org/epg/cs";
        let html = get_html(url).await?;

        let printer = Box::new(CsTv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for CsTv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let current_selector = Selector::parse("li.sc-current").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            match ul.select(&current_selector).next() {
                Some(current) => {
                    if let Some(title) = current.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {}",
                            channels[i].color(CSCOLOR),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
                None => writeln!(buf, "{} 現在放送していません", channels[i]).unwrap(),
            }
        }
    }
}

pub struct TodayCsTv {
    epg_doc: Html,
}

impl TodayCsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let url = "https://bangumi.org/epg/cs";
        let html = get_html(url).await?;

        let printer = Box::new(TodayCsTv { epg_doc: html });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for TodayCsTv {
    fn print(&self, w: T) {
        let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
        let channels = self
            .epg_doc
            .select(&ch_selector)
            .map(|e| e.inner_html())
            .collect::<Vec<_>>();
        let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

        let program_selector = Selector::parse("div#program_area ul").unwrap();
        let future_selector = Selector::parse("li.sc-future").unwrap();
        let title_selector = Selector::parse("p.program_title").unwrap();

        let program_area = self.epg_doc.select(&program_selector);
        let mut buf = io::BufWriter::new(w);
        for (i, ul) in program_area.enumerate() {
            writeln!(buf, "{}", channels[i].color(CSCOLOR)).unwrap();
            for li in ul.select(&future_selector) {
                let start = li.value().attr("s").unwrap();
                let start_hours = start.get(8..10).unwrap();
                let start_minutes = start.get(10..12).unwrap();
                let end = li.value().attr("e").unwrap();
                let end_hours = end.get(8..10).unwrap();
                let end_minutes = end.get(10..12).unwrap();
                if let Some(title) = li.select(&title_selector).next() {
                    writeln!(
                        buf,
                        "{}:{} ~ {}:{} {}",
                        start_hours,
                        start_minutes,
                        end_hours,
                        end_minutes,
                        unescape(title.inner_html())
                    )
                    .unwrap();
                }
            }
        }
    }
}

pub struct WeekCsTv {
    epg_docs: Vec<Html>,
}

impl WeekCsTv {
    pub async fn init<T: Write>() -> Result<Box<dyn Printer<T>>> {
        let mut datetime = Local::now();
        if datetime.hour() < TV_GUIDE_START_TIME {
            datetime = Local::now() + Duration::days(-1);
        }
        const WEEK_COUNT: usize = 8;
        let mut urls: [String; WEEK_COUNT] = Default::default();
        for index in urls.iter_mut().take(WEEK_COUNT) {
            let today = datetime.format("%Y%m%d");
            let url = format!("https://bangumi.org/epg/cs?broad_cast_date={}", today);
            *index = url;
            datetime += Duration::days(1);
        }
        let htmls = async_get_htmls(urls.to_vec()).await?;
        let printer = Box::new(WeekCsTv { epg_docs: htmls });

        Ok(printer)
    }
}

impl<T: Write> Printer<T> for WeekCsTv {
    fn print(&self, w: T) {
        let mut buf = io::BufWriter::new(w);
        for epg_doc in &self.epg_docs {
            let ch_selector = Selector::parse("div#ch_area ul li.topmost p").unwrap();
            let channels = epg_doc
                .select(&ch_selector)
                .map(|e| e.inner_html())
                .collect::<Vec<_>>();
            let channels = channels.iter().map(|s| s.trim()).collect::<Vec<_>>();

            let program_selector = Selector::parse("div#program_area ul").unwrap();
            let future_selector = Selector::parse("li.sc-future").unwrap();
            let title_selector = Selector::parse("p.program_title").unwrap();

            let program_area = epg_doc.select(&program_selector);
            for (i, ul) in program_area.enumerate() {
                for li in ul.select(&future_selector) {
                    let start = li.value().attr("s").unwrap();
                    let end = li.value().attr("e").unwrap();
                    let start = NaiveDateTime::parse_from_str(start, "%Y%m%d%H%M").unwrap();
                    let end = NaiveDateTime::parse_from_str(end, "%Y%m%d%H%M").unwrap();
                    if let Some(title) = li.select(&title_selector).next() {
                        writeln!(
                            buf,
                            "{} {} ~ {} {}",
                            channels[i],
                            start.format("%a %R"),
                            end.format("%a %R"),
                            unescape(title.inner_html())
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
}

async fn get_html(url: &str) -> Result<Html> {
    let s = get_response_body_string(url).await?;
    let html = Html::parse_document(&s);
    Ok(html)
}

async fn get_response_body_string(url: &str) -> Result<String> {
    let client: Client = Config::new()
        .set_http_keep_alive(HTTP_KEEP_ALIVE)
        .try_into()?;
    let req = surf::get(url);
    let rbs = client
        .recv_string(req)
        .await
        .map_err(|err| anyhow!(err))
        .context("Failed to fetch from bangumi.org")?;

    Ok(rbs)
}

async fn multiple_requests(urls: Vec<String>) -> Vec<Result<String>> {
    let mut handles = vec![];
    for url in urls {
        handles.push(task::spawn_local(async move {
            get_response_body_string(&url).await
        }));
    }

    let mut body_strings = vec![];
    for handle in handles {
        body_strings.push(handle.await);
    }

    body_strings
}

async fn async_get_htmls(urls: Vec<String>) -> Result<Vec<Html>> {
    let results = multiple_requests(urls).await;
    let res_bodies = results.into_iter().collect::<Result<Vec<String>>>()?;
    let htmls = res_bodies
        .iter()
        .map(|b| Html::parse_document(b))
        .collect::<Vec<Html>>();
    Ok(htmls)
}

#[allow(dead_code)]
async fn print_execution_time<F, Fut, T, U>(arg: U, f: F) -> T
where
    F: Fn(U) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f(arg).await;
    let elapsed = start.elapsed();
    println!("Elapsed time: {:.2?}", elapsed);
    result
}
