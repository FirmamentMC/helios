use std::
	time::{Duration, SystemTime, UNIX_EPOCH}
;

use tokio::sync::Mutex;
use twilight_http::{Client, request::channel::reaction::RequestReactionType};
use twilight_model::{
	channel::Message,
	gateway::payload::incoming::{MessageCreate, MessageDelete},
	id::{
		Id,
		marker::{MessageMarker, UserMarker},
	},
	util::Timestamp,
};

use crate::{
	EventWithContext, handle, handle_message,
	utils::consts::{COUNTING_CHANNEL, FIRMAMENT_SERVER, THE_NO_ONE},
};

handle_message!(should_reply, on_count);
handle!(MessageDelete, on_delete);
async fn on_delete(event: EventWithContext<&MessageDelete>) -> eyre::Result<()> {
	if event.channel_id != COUNTING_CHANNEL {
		return Ok(());
	}

	let current_holder = CURRENT_COUNT.lock().await;
	let Some(current) = &*current_holder else {
		return Ok(());
	};
	if current.message_id == event.id {
		let message = format!(
			"{} — Hi, it is me — cute little mouse — and i am here to provide you with some help. It has come to my attention that recently someone has deleted a message in this channel. Not to worry, I have remembered their number. <@{}> recently posted {}.",
			current.count, current.user, current.count
		);
		event
			.client
			.create_message(COUNTING_CHANNEL)
			.content(&message)
			.await?;
		mute(&event.client, current.user, Duration::from_days(1)).await?;
	}

	Ok(())
}

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
				.filter(|it| it.id != event.id)
				.filter_map(extract_number)
				.next()
				.unwrap_or(LastNumber {
					user: THE_NO_ONE,
					count: 0,
					message_id: Id::new(1),
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

async fn mute(client: &Client, id: Id<UserMarker>, duration: Duration) -> eyre::Result<()> {
	let mute_until = (Timestamp::from_secs(
		(SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_secs()
			+ duration.as_secs()) as i64,
	))?;
	client
		.update_guild_member(FIRMAMENT_SERVER, id)
		.communication_disabled_until(Some(mute_until))
		.await?;
	Ok(())
}

async fn punish(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	event
		.client
		.delete_message(event.channel_id, event.id)
		.await?;
	mute(&event.client, event.author.id, Duration::from_hours(1)).await?;
	Ok(())
}

fn extract_number(msg: &Message) -> Option<LastNumber> {
	let number = msg
		.content
		.split_whitespace()
		.next()
		.and_then(|it| it.parse::<u64>().ok())?;
	Some(LastNumber {
		user: msg.author.id,
		count: number,
		message_id: msg.id,
	})
}

#[derive(Clone, Debug)]
struct LastNumber {
	user: Id<UserMarker>,
	count: u64,
	message_id: Id<MessageMarker>,
}

static CURRENT_COUNT: Mutex<Option<LastNumber>> = Mutex::const_new(None);
