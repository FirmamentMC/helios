use std::sync::Arc;

use cached::proc_macro::cached;
use twilight_http::Client;
use twilight_model::{
	guild::Permissions,
	id::{Id, marker::RoleMarker},
};

use crate::{HeliosCache, utils::consts::FIRMAMENT_SERVER};

/// Create or find a role by name. Shall be used purely for vanity labels, not any important roles.
///
/// I rely on time based caching of this function to deal with some of discords eventual consistency and network delay.
///
/// I accept this function being scuffed; if it is unreliable i can accept the failure modes.
#[cached(key = "Arc<str>", convert = "{ name.clone() }")]
pub async fn upsert_vanity_role(
	client: &Client,
	hcache: &HeliosCache,
	name: Arc<str>,
) -> Id<RoleMarker> {
	let roles = hcache.guild_roles(FIRMAMENT_SERVER).unwrap();
	let existing_role = roles.iter().find(|&it| {
		let role = hcache.role(*it).unwrap();
		role.name == *name && role.permissions.is_empty()
	});
	match existing_role {
		Some(&role) => role,
		None => {
			client
				.create_role(FIRMAMENT_SERVER)
				.name(&name)
				.permissions(Permissions::empty())
				.mentionable(false)
				.await
				.unwrap()
				.model()
				.await
				.unwrap()
				.id
		}
	}
}
