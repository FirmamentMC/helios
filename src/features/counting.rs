use std::time::Duration;

use tokio::sync::Mutex;
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::{
	channel::Message,
	gateway::payload::incoming::MessageCreate,
	id::{Id, marker::UserMarker},
	util::Timestamp,
};

use crate::{
	handle_message, utils::
		consts::{COUNTING_CHANNEL, FIRMAMENT_SERVER, THE_NO_ONE}
	, EventWithContext
};

async fn on_count(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	if event.channel_id != COUNTING_CHANNEL {
		return Ok(());
	}

	let Some(given) = extract_number(&event) else {
		return punish(event).await;
	};

	let mut current_holder = CURRENT_COUNT.lock().await;
	let current = match &*current_holder {
		Some(x) => {
			tracing::info!("Found existing counter {:?}", x);
			x.clone()
		}
		None => {
			let messages = event
				.client
				.channel_messages(COUNTING_CHANNEL)
				.await?
				.model()
				.await?;
			let new = messages
				.iter()
				.filter(|it|it.id != event.id)
				.filter_map(extract_number)
				.next()
				.unwrap_or(LastNumber {
					user: THE_NO_ONE,
					count: 0,
				});
			tracing::info!("Loaded counter from channel {:?}", new);
			new
		}
	};

	if given.count != current.count + 1 || given.user == current.user {
		drop(current_holder);
		return punish(event).await;
	}
	tracing::info!("Incrementing counter to {given:?}");
	current_holder.replace(given);
	drop(current_holder);

	event
		.client
		.create_reaction(
			event.channel_id,
			event.id,
			&RequestReactionType::Unicode { name: "🔢" },
		)
		.await?;
	// TODO: maybe react?

	Ok(())
}

async fn punish(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	event
		.client
		.delete_message(event.channel_id, event.id)
		.await?;
	let mute_until =
		(Timestamp::from_secs(event.timestamp.as_secs() + PUNISH_DURATION.as_secs() as i64))?;
	event
		.client
		.update_guild_member(FIRMAMENT_SERVER, event.author.id)
		.communication_disabled_until(Some(mute_until))
		.await?;
	Ok(())
}

const PUNISH_DURATION: Duration = Duration::from_hours(1);

fn extract_number(msg: &Message) -> Option<LastNumber> {
	let number = msg
		.content
		.split_whitespace()
		.next()
		.and_then(|it| it.parse::<u64>().ok())?;
	Some(LastNumber {
		user: msg.author.id,
		count: number,
	})
}

#[derive(Clone, Debug)]
struct LastNumber {
	user: Id<UserMarker>,
	count: u64,
}

static CURRENT_COUNT: Mutex<Option<LastNumber>> = Mutex::const_new(None);

handle_message!(should_reply, on_count);
