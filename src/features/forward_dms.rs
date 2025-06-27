use tracing::info;
use twilight_model::{channel::message::{MessageReference, MessageReferenceType}, gateway::payload::incoming::MessageCreate, id::Id};

use crate::{handle, utils::UserExt as _, EventWithContext};

async fn on_message(context: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	let message = context.event;
	if message.guild_id.is_none() {
		info!("Forwarding DM from {}", message.author.name);
		let dm_channel_id = Id::new(1386643549071872031);
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
			.create_message(dm_channel_id)
			.payload_json(&payload)
			.await?
			.model()
			.await?;
		context
			.client
			.create_message(dm_channel_id)
			.content(&format!("Received DM from {}", message.author.mention()))
			.reply(forwarded_message.id)
			.await?;
	}
	Ok(())
}

handle!(MessageCreate, on_message);
