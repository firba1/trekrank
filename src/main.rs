extern crate serde_json;
extern crate params;
extern crate logger;
extern crate env_logger;

#[macro_use] extern crate askama;
#[macro_use] extern crate iron;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate error_chain;

use askama::Template;
use iron::Iron;
use iron::IronResult;
use iron::headers::ContentType;
use iron::middleware::Chain;
use iron::request::Request;
use iron::response::Response;
use iron::status;
use params::Params;
use params::Value;
use iron::Plugin;
use std::env;
use logger::Logger;

#[derive(Serialize, Deserialize)]
struct Episode {
    season: u8,
    title: String,
    link: String,
    episode_num: String,
    description: String,
    series: String,
}

struct RankedEpisode {
    rank: u16,
    episode: Episode,
}

struct SeasonPresenter {
    number: String,
    display: String,
    selected: bool,
}

struct Series<'a> {
    value: &'a str,
    name: &'a str,
}

struct SeriesPresenter<'a> {
    series: Series<'a>,
    selected: bool,
}

#[derive(Template)]
#[template(path="app.tmpl.html")]
struct App<'a> {
    episodes: Vec<RankedEpisode>,
    show_description: bool,
    seasons: Vec<SeasonPresenter>,
    show_rank: bool,
    series_list: Vec<SeriesPresenter<'a>>,
}

mod error {
    error_chain!{}
}

use error::ResultExt;

struct AppParams {
    show_description: bool,
    season_filter: Option<u8>,
    series_filter: Option<String>,
}

fn get_app_params(raw_params: &params::Map) -> Result<AppParams, error::Error> {
    let show_description = raw_params.find(&["description"]).map_or(
        Ok(false),
        |ref value| -> Result<bool, error::Error> {
            match value {
                &&Value::String(ref string) => {
                    if string == "show" {
                        Ok(true)
                    } else {
                        Err("invalid value for description".into())
                    }
                }
                _ => Err("invalid type for description".into())
            }
        },
    )?;

    let season_filter: Option<u8> = raw_params.find(&["season"]).map_or(
        Ok(None),
        |ref value| -> Result<Option<u8>, error::Error> {
            match value {
                &&Value::String(ref string) => {
                    if string == "" {
                        return Ok(None)
                    }
                    let num = string.parse().chain_err(|| "parse error")?;
                    if num >= 1 && num <= 7 {
                        Ok(Some(num))
                    } else {
                        Err("invalid season".into())
                    }
                }
                _ => Ok(None),
            }
        }
    )?;

    let series_filter: Option<String> = raw_params.find(&["series"]).map_or(
        Ok(None),
        |ref value| -> error::Result<Option<String>> {
            match value {
                &&Value::String(ref string) => {
                    let string = string.clone();
                    if vec!["TNG", "DS9", "Voyager"].contains(&string.as_str()) {
                        Ok(Some(string))
                    } else if string == ""{
                        Ok(None)
                    } else {
                        Err(format!("invalid season '{}'", string).into())
                    }
                },
                _ => Err("series is wrong type".into()),
            }
        },
    )?;


    Ok(AppParams{
        show_description: show_description,
        season_filter: season_filter,
        series_filter: series_filter,
    })
}

fn get_series_list<'a>(series_filter: Option<String>) -> Vec<SeriesPresenter<'a>> {
    vec![
        SeriesPresenter{
            series: Series{value: "", name: "All Series"},
            selected: series_filter.is_none(),
        },
    ].into_iter().chain(
        vec![
            Series{value: "TNG", name: "The Next Generation"},
            Series{value: "DS9", name: "Deep Space 9"},
            Series{value: "Voyager", name: "Voyager"},
        ].into_iter().map(|thing| {
            let value = thing.value.clone();
            SeriesPresenter{
                series: thing,
                selected: series_filter.clone().map_or(
                    false,
                    |inner_series| inner_series == value,
                ),
            }
        })
    ).collect()
}

fn get_seasons(season_filter: Option<u8>) -> Vec<SeasonPresenter> {
    vec![SeasonPresenter{
        number: "".to_string(),
        display: "All Seasons".to_string(),
        selected: season_filter.is_none(),
    }].into_iter().chain(
        vec![1, 2, 3, 4, 5, 6, 7].into_iter().map(
            |num| SeasonPresenter{
                number: num.to_string(),
                display: format!("Season {}", num),
                selected: if let Some(season) = season_filter {
                    season == num
                } else { false }
            }
        ),
    ).collect()
}


fn app(req: &mut Request) -> IronResult<Response> {
    let params = req.get::<Params>().unwrap();

    let AppParams{
        show_description,
        season_filter,
        series_filter,
    } = itry!(get_app_params(&params));

    let rankings_json = include_str!("star_trek_rank.json");
    let episodes: Vec<Episode> = itry!(serde_json::from_str(rankings_json));
    let episodes: Vec<RankedEpisode> = episodes.into_iter().enumerate().map(
        |(rank, episode)| RankedEpisode{
            rank: (rank + 1) as u16,
            episode: episode,
        }
    ).collect();
    let episodes: Vec<RankedEpisode> = episodes.into_iter().filter(
        |episode| season_filter.map_or(
            true,
            |season| episode.episode.season == season,
        )
    ).filter(
        |episode| series_filter.clone().map_or(
            true,
            |series| episode.episode.series == series,
        ),
    ).collect();

    let series_list = get_series_list(series_filter.clone());
    let seasons = get_seasons(season_filter);

    let show_rank = season_filter.is_some() || series_filter.is_some();

    let mut response = Response::with((
        status::Ok,
        itry!(App{
            episodes: episodes,
            show_description: show_description,
            seasons: seasons,
            show_rank: show_rank,
            series_list: series_list,
        }.render())
    ));
    response.headers.set(ContentType::html());
    Ok(response)
}

fn main() {
    env_logger::init();
    let mut chain = Chain::new(app);
    let (logger_before, logger_after) = Logger::new(None);
    chain.link_before(logger_before);
    chain.link_after(logger_after);

    let port: u16 = env::var("PORT").ok().and_then(
        |port| port.parse().ok()
    ).unwrap_or(3000);
    Iron::new(chain).http(format!("0.0.0.0:{}", port)).unwrap();
}
