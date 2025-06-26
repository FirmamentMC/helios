use std::{env, sync::Arc};

use eyre::Context as _;
use tracing::info;
use twilight_gateway::{ConfigBuilder, Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client;
use twilight_model::{
    channel::message::{AllowedMentions, MessageReference, MessageReferenceType},
    gateway::{
        payload::outgoing::update_presence::UpdatePresencePayload,
        presence::{Activity, ActivityType, Status},
    },
    id::Id,
};

use crate::utils::UserExt as _;

pub mod utils;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt::init();
    _ = dotenv::dotenv();
    let token = env::var("DISCORD_TOKEN").wrap_err("Missing DISCORD_TOKEN env var")?;
    let intents = Intents::MESSAGE_CONTENT | Intents::DIRECT_MESSAGES;

    let client = Client::builder()
        .token(token.clone())
        .default_allowed_mentions(AllowedMentions::default())
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

        let client = client.clone();
        tokio::task::spawn(async move {
            match handle(EventContext {
                event,
                client: client,
            })
            .await
            {
                Ok(()) => (),
                Err(err) => tracing::error!(?err, "failed to handle event"),
            }
        });
    }

    Ok(())
}

struct EventContext {
    event: Event,
    client: Arc<Client>,
}

async fn handle(context: EventContext) -> eyre::Result<()> {
    match &context.event {
        Event::MessageCreate(message) => {
            if message.guild_id.is_none() {
                info!("Forwarding DM from {}", message.author.name);
                let DM_CHANNEL_ID = Id::new(1386643549071872031);
                let payload = serde_json::to_vec(&serde_json::json!({
                    "content": "",
                    "message_reference": (serde_json::to_value(MessageReference {
                        channel_id: Some(message.channel_id),
                        message_id: Some(message.id),
                        guild_id: message.guild_id,
                        kind: MessageReferenceType::Forward,
                        fail_if_not_exists: Some(true),
                    })?),
                }))?;
                let forwarded_message = context
                    .client
                    .create_message(DM_CHANNEL_ID)
                    .payload_json(&payload)
                    .await?
                    .model()
                    .await?;
                context
                    .client
                    .create_message(DM_CHANNEL_ID)
                    .content(&format!("Received DM from {}", message.author.mention()))
                    .reply(forwarded_message.id)
                    .await?;
            }
        }
        _otherwise => {}
    };
    Ok(())
}
