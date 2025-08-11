use std::sync::Arc;

use chrono::{DateTime, Datelike, Local, NaiveTime, Timelike, Utc, offset::LocalResult};
use chrono_tz::Tz;
use csv::StringRecord;
use eyre::Context;
use positioned_io::RandomAccessFile;
use rc_zip_tokio::ReadZip;
use tokio::sync::OnceCell;
use twilight_model::gateway::payload::incoming::MessageCreate;
use unicase::UniCase;

use crate::{EventWithContext, handle_message, utils::cached};
handle_message!(should_reply, on_post_time);

async fn on_post_time(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	if event.content == "!time" {
		post_time(event, "frankfurt").await?;
	} else if let Some(rest) = event.content.strip_prefix("!time ") {
		post_time(event, rest).await?;
	}
	Ok(())
}

async fn post_time(
	event: EventWithContext<&MessageCreate>,
	search_phrase: &str,
) -> eyre::Result<()> {
	let searcher = MultiNamePrefixMatcher::new(search_phrase);
	let geo_db = geo_database().await;
	let mut candidates = geo_db
		.iter()
		.filter(|it| {
			let mut item_searcher = searcher.clone();
			for ele in &it.match_names {
				item_searcher.accept_casefolded_match(&ele);
			}
			item_searcher.is_matched()
		}) // mfw treemaps exist, but also compute is cheap
		.collect::<Vec<_>>();
	candidates.sort_by_key(|it| it.population);
	candidates.reverse();
	candidates.truncate(1);
	let mut text = String::new();
	for city in candidates {
		let current_time_in_timezone = Utc::now().with_timezone(&city.timezone);
		let mut f = format!(
			"**{}** - [{}](<http://time.is/{}>)\nCurrent Time: {:02}.{:02}.{:04} **{:02}:{:02}**\n",
			city.name,
			city.timezone,
			urlencoding::encode(&city.name),
			current_time_in_timezone.day(),
			current_time_in_timezone.month(),
			current_time_in_timezone.year(),
			current_time_in_timezone.hour(),
			current_time_in_timezone.minute(),
		);
		f += &match current_time_in_timezone.with_time(NaiveTime::MIN) {
			LocalResult::Single(timestamp) => {
				format!("-# Their midnight is your <t:{}:t>\n", timestamp.timestamp())
			}
			LocalResult::Ambiguous(a, b) => format!(
				"-# Their midnights (??? i thought timezone switches happened only at like 02:00, not midnight) are <t:{}:t> and <t:{}:t>\n",
				a.timestamp(),
				b.timestamp()
			),
			LocalResult::None => {
				"-# They don't have a midnight (??? i thought timezone switches happened only at like 02:00, not midnight)\n".to_owned()
			}
		};

		if f.len() + text.len() > 2000 {
			break;
		}
		text += &f;
	}
	event.reply().content(&text).await?;

	Ok(())
}

#[derive(Clone, Debug)]
struct MultiNamePrefixMatcher {
	expected_matches: Vec<Arc<str>>,
}

impl MultiNamePrefixMatcher {
	fn new(text: &str) -> MultiNamePrefixMatcher {
		MultiNamePrefixMatcher {
			expected_matches: text
				.split_whitespace()
				.filter(|it| !it.is_empty())
				.map(|it| UniCase::new(it).to_folded_case().into())
				.collect(),
		}
	}

	fn is_matched(&self) -> bool {
		self.expected_matches.is_empty()
	}

	fn accept_casefolded_match(&mut self, obj: &str) {
		for ele in obj.split_whitespace() {
			self.expected_matches.retain(|it| !ele.starts_with(it.as_ref()));
		}
	}
}

fn parse_record(record: &StringRecord) -> eyre::Result<GeoEntry> {
	let name = record[1].to_owned();
	let alternatenames: Vec<String> = record[3].split(",").map(ToOwned::to_owned).collect();
	let match_names = vec![&name]
		.iter()
		.copied()
		.chain(alternatenames.iter())
		.map(|it| UniCase::new(it).to_folded_case())
		.collect();
	Ok(GeoEntry {
		geonameid: record[0].parse().context("parsing geonameid")?,
		name,
		alternatenames,
		latitude: record[4].parse().context("parsing latitude")?,
		longitude: record[5].parse().context("parsing longitude")?,
		timezone: record[17].parse().context("parsing timezone")?,
		population: record[14].parse().context("parsing population")?,
		match_names,
	})
}

async fn geo_database() -> &'static [GeoEntry] {
	static GEO_ENTRY: OnceCell<Vec<GeoEntry>> = tokio::sync::OnceCell::const_new();
	GEO_ENTRY
		.get_or_init(|| async {
			let zip_path =
				cached::download_url("https://download.geonames.org/export/dump/cities15000.zip")
					.await
					.unwrap();
			let zip_file = Arc::new(RandomAccessFile::open(zip_path).unwrap());
			let zip_archive = ReadZip::read_zip(&zip_file).await.unwrap();
			let cities_entry = zip_archive.entries().next().unwrap(); // Each zip contains exactly one file
			let cities_bytes = cities_entry.bytes().await.unwrap();
			let csv_reader = csv::ReaderBuilder::default()
				.has_headers(false)
				.delimiter(b'\t')
				.from_reader(&cities_bytes[..]);

			let mut v = Vec::new();
			for record in csv_reader.into_records() {
				let record = record.unwrap();
				match parse_record(&record) {
					Ok(entry) => v.push(entry),
					Err(err) => tracing::error!(
						?err,
						"Failed to load city entry from {}",
						record.as_slice()
					),
				}
			}
			v
		})
		.await
}
// csv_reader.set_headers(StringRecord::from(vec![
// 0	"geonameid",
// 1	"name",
// 2	"asciiname",
// 3	"alternatenames",
// 4	"latitude",
// 5	"longitude",
// 6	"feature class",
// 7	"feature code",
// 8	"country code",
// 9	"cc2",
// 10	"admin1 code",
// 11	"admin2 code",
// 12	"admin3 code",
// 13	"admin4 code",
// 14	"population",
// 15	"elevation",
// 16	"dem",
// 17	"timezone",
// 18	"modification date",
// ]));
#[derive(Clone, Debug)]
#[allow(unused)]
struct GeoEntry {
	geonameid: usize,
	name: String,
	alternatenames: Vec<String>,
	timezone: Tz,
	match_names: Vec<String>,
	population: u32,
	latitude: f32,
	longitude: f32,
}
