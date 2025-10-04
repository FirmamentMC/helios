use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
	utils::{
		consts::{COUNTING_CHANNEL, FIRMAMENT_SERVER, THE_NO_ONE},
		dynroles::upsert_vanity_role,
	},
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
					number_format: NumberFormat::Decimal,
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
	let format = given.number_format;
	let count = given.count;
	current_holder.replace(given);
	drop(current_holder);

	event
		.client
		.create_reaction(
			event.channel_id,
			event.id,
			&RequestReactionType::Unicode {
				name: match format {
					NumberFormat::Decimal => "🔢",
					_ => "🤓",
				},
			},
		)
		.await?;

	let counting_role = upsert_vanity_role(
		&event.client,
		&event.cache,
		format!("counting: {}", next_power_of_ten(count)).into(),
	)
	.await;

	let has_role = if let Some(member) = event.cache.member(FIRMAMENT_SERVER, event.author.id) {
		member.roles().contains(&counting_role)
	} else {
		false
	};
	if !has_role {
		event
			.client
			.add_guild_member_role(FIRMAMENT_SERVER, event.author.id, counting_role)
			.await?;
	}

	Ok(())
}

const fn next_power_of_ten(c: u64) -> u64 {
	if c == 0 {
		return 0;
	}
	let mut p = 1;
	while p < c {
		p *= 10;
	}
	p
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
	let (number, number_format) = msg
		.content
		.split_whitespace()
		.next()
		.and_then(parse_number)?;
	Some(LastNumber {
		user: msg.author.id,
		count: number,
		number_format,
		message_id: msg.id,
	})
}

fn parse_number(text: &str) -> Option<(u64, NumberFormat)> {
	if let Some(hex) = text.strip_prefix("0x") {
		Some((
			u64::from_str_radix(hex, 16).ok()?,
			NumberFormat::Hexadecimal,
		))
	} else if let Some(hex) = text.strip_suffix("h") {
		Some((
			u64::from_str_radix(hex, 16).ok()?,
			NumberFormat::Hexadecimal,
		))
	} else if let Some(oct) = text.strip_prefix("0o") {
		Some((u64::from_str_radix(oct, 8).ok()?, NumberFormat::Octal))
	} else if let Some(binary) = text.strip_prefix("0b") {
		Some((u64::from_str_radix(binary, 2).ok()?, NumberFormat::Binary))
	} else if let Some(unary) = text.strip_prefix("0u") {
		if unary.chars().any(|it| it != '0') {
			None
		} else {
			Some((unary.len() as u64, NumberFormat::Unary))
		}
	} else {
		Some((u64::from_str_radix(text, 10).ok()?, NumberFormat::Decimal))
	}
}

#[derive(Clone, Debug)]
struct LastNumber {
	user: Id<UserMarker>,
	count: u64,
	message_id: Id<MessageMarker>,
	number_format: NumberFormat,
}

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
enum NumberFormat {
	Decimal,
	Binary,
	Hexadecimal,
	Unary,
	Octal,
}

#[cfg(test)]
mod tests {
	use crate::features::counting::{NumberFormat, parse_number};

	#[test]
	fn test_basic_number_parser() {
		assert_eq!(parse_number("1"), Some((1, NumberFormat::Decimal)));
		assert_eq!(parse_number("0b10"), Some((2, NumberFormat::Binary)));
		assert_eq!(
			parse_number("0x10"),
			Some((0x10, NumberFormat::Hexadecimal))
		);
		assert_eq!(parse_number("0o10"), Some((8, NumberFormat::Octal)));
		assert_eq!(parse_number("0u000"), Some((3, NumberFormat::Unary)));
	}
}

static CURRENT_COUNT: Mutex<Option<LastNumber>> = Mutex::const_new(None);
