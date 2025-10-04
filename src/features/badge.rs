use twilight_http::request::AuditLogReason;
use twilight_mention::Mention;
use twilight_model::gateway::payload::incoming::MessageCreate;

use crate::{
	EventWithContext, handle_message,
	utils::{args, consts::FIRMAMENT_SERVER, dynroles::upsert_vanity_role},
};

handle_message!(should_obey, on_badge_cmd);

async fn on_badge_cmd(event: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	if let Some(args) = event.content.strip_prefix("!badge ") {
		let Some((user, name)) = args::chomp_user(args) else {
			return Ok(());
		};
		let role = upsert_vanity_role(&event.client, &event.cache, name.to_owned().into()).await;
		let message = format!("Added badge role {} to {}", role.mention(), user.mention());
		let command = format!("badge added by {}", event.author.id.get());
		event
			.client
			.add_guild_member_role(FIRMAMENT_SERVER, user, role)
			.reason(&command)
			.await?;
		event.reply().content(&message).await?;
	} else if let Some(args) = event.content.strip_prefix("!badge ") {
		let Some((user, name)) = args::chomp_user(args) else {
			return Ok(());
		};
		let member = event.cache.member(FIRMAMENT_SERVER, user).unwrap();
		let Some(role) = member.roles().iter().find(|rid| {
			let role = event.cache.role(**rid).unwrap();
			role.permissions.is_empty() && role.name == name
		}) else {
			let message = format!("Could not find badge `{}` on {}", name, user.mention());
			event.reply().content(&message).await?;
			return Ok(());
		};

		let command = format!("badge removed by {}", event.author.id.get());
		let message = format!(
			"Deleted badge role {} from {}",
			role.mention(),
			user.mention()
		);
		event
			.client
			.remove_guild_member_role(FIRMAMENT_SERVER, user, *role)
			.reason(&command)
			.await?;
		event.reply().content(&message).await?;
	}
	Ok(())
}
