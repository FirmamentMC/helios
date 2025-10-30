use std::collections::HashMap;

use eyre::OptionExt;
use octocrab::models::IssueState;
use tracing::debug;
use twilight_model::{gateway::payload::incoming::MessageCreate, util::Timestamp};
use twilight_util::builder::embed::{
	EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, ImageSource,
};

use crate::{EventWithContext, fixed_regex, handle_message, utils::consts::FIRMAMENT_REPO};

handle_message!(should_reply, on_issue);

async fn on_issue(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	fixed_regex!(ISSUE_HASH = "#([0-9]+)");
	debug!("checking {} for issue hashes", event.content);
	for m in ISSUE_HASH.captures_iter(&event.content) {
		let issue_number = (m.get(1).unwrap()).as_str().parse()?;
		debug!("found issue #{}", issue_number);
		let issue = octocrab::instance()
			.issues_by_id(FIRMAMENT_REPO)
			.get(issue_number)
			.await?;
		debug!("downloaded issue #{}", issue_number);
		let author = EmbedAuthorBuilder::new(format!("@{}", issue.user.login))
			.icon_url(ImageSource::url(issue.user.avatar_url)?)
			.url(issue.user.html_url);
		let body = issue.body.ok_or_eyre("no issue body")?;
		let parts = parse_md_sections(&body);
		let mut embed = EmbedBuilder::new()
			.author(author)
			.title(format!("#{} - {}", issue.number, issue.title))
			.url(issue.html_url)
			.timestamp(Timestamp::from_secs(issue.created_at.timestamp())?);
		if let Some(_) = issue.pull_request {
			embed = embed.description(body).color(match issue.state {
				IssueState::Open => 0x3040ff,
				IssueState::Closed => 0xff40e0,
				_ => 0,
			});
		} else {
			let version = parts.get("Firmament Version").unwrap_or(&"not specified");
			embed = embed
				.field(EmbedFieldBuilder::new(
					"Firmament Version",
					version.to_owned(),
				))
				.description(parts["Bug Description"])
				.color(match issue.state {
					IssueState::Open => 0xff6030,
					IssueState::Closed => 0x30ff30,
					_ => 0,
				});
		}
		debug!("sending embed");
		event.reply().embeds(&vec![embed.build()]).await?;
	}

	Ok(())
}
fn parse_md_sections<'a>(source: &'a str) -> HashMap<&'a str, &'a str> {
	fixed_regex!(HEADER = "#+ *([^\n]+)\n((?:[^\n]|\n+[^\n#])+)");
	let mut map = HashMap::new();
	for m in HEADER.captures_iter(source) {
		map.insert(m.get(1).unwrap().as_str(), m.get(2).unwrap().as_str());
	}
	map
}
