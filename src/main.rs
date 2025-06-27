#![deny(clippy::missing_const_for_fn)]
#![feature(iter_intersperse)]

use std::{env, ops::Deref, sync::Arc};

use eyre::Context as _;
use twilight_gateway::{
	ConfigBuilder, Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt,
};
use twilight_http::{request::channel::message::CreateMessage, Client};
use twilight_model::{
	channel::message::AllowedMentions,
	gateway::{
		payload::{incoming::MessageCreate, outgoing::update_presence::UpdatePresencePayload},
		presence::{Activity, ActivityType, Status},
	},
};

use crate::utils::{BoxedEventHandler, MessageExt as _};

pub mod utils;

#[tokio::main]
async fn main() -> eyre::Result<()> {
	tracing_subscriber::fmt::init();
	_ = dotenv::dotenv();
	let token = env::var("DISCORD_TOKEN").wrap_err("Missing DISCORD_TOKEN env var")?;
	let intents = Intents::MESSAGE_CONTENT | Intents::DIRECT_MESSAGES | Intents::GUILD_MESSAGES;

	let client = Client::builder()
		.token(token.clone())
		.default_allowed_mentions(AllowedMentions {
			replied_user: true,
			..AllowedMentions::default()
		})
		.build();
	let client = Arc::new(client);

	let config = ConfigBuilder::new(token, intents)
		.presence(UpdatePresencePayload::new(
			vec![Activity {
				application_id: None,
				assets: None,
				buttons: vec![],
				created_at: None,
				details: None,
				emoji: None,
				flags: None,
				id: None,
				instance: None,
				kind: ActivityType::Custom,
				name: "Ratting people".to_owned(),
				party: None,
				secrets: None,
				state: None,
				timestamps: None,
				url: None,
			}],
			false,
			None,
			Status::DoNotDisturb,
		)?)
		.build();
	let mut shard = Shard::with_config(ShardId::ONE, config);

	while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
		let event = match item {
			Ok(event) => event,
			Err(err) => {
				tracing::error!(?err, "Failed to receive event");
				continue;
			}
		};
		let event = Arc::new(event);

		for handler in inventory::iter::<BoxedEventHandler>::iter() {
			let client = client.clone();
			let event = event.clone();
			let context = EventContext { event, client };
			tokio::task::spawn(async move {
				match handler.handle(context).await {
					Ok(()) => (),
					Err(err) => tracing::error!(?err, "failed to handle event"),
				}
			});
		}
	}

	Ok(())
}

pub mod features;

pub struct EventWithContext<T> {
	pub event: T,
	pub client: Arc<Client>,
}

impl<T> Deref for EventWithContext<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.event
	}
}

impl<T> EventWithContext<T> {
	pub fn new(event: T, client: Arc<Client>) -> EventWithContext<T> {
		EventWithContext { event, client }
	}
	pub fn replace<N>(self, value: N) -> EventWithContext<N> {
		EventWithContext {
			event: value,
			client: self.client,
		}
	}
}

impl EventWithContext<&MessageCreate> {
	pub fn reply(&self) -> CreateMessage<'_> {
		self
			.client
			.create_message(self.channel_id)
			.reply(self.reply_to_reply())
	}
}

type EventContext = EventWithContext<Arc<Event>>;
