use std::pin::Pin;

use twilight_model::{
	channel::{
		Message,
		message::{MessageReference, MessageReferenceType},
	},
	gateway::payload::incoming::MessageCreate,
	id::{
		Id,
		marker::{GuildMarker, MessageMarker, RoleMarker},
	},
	user::User,
};

use crate::{EventContext, EventWithContext};

pub trait UserExt {
	fn mention(&self) -> String;
}
impl UserExt for User {
	fn mention(&self) -> String {
		format!("<@{}>", self.id)
	}
}

pub trait MessageExt {
	fn reply_to_reply(&self) -> Id<MessageMarker>;
}

impl MessageExt for Message {
	fn reply_to_reply(&self) -> Id<MessageMarker> {
		match self.reference {
			Some(MessageReference {
				kind: MessageReferenceType::Default,
				message_id: Some(reply_id),
				..
			}) => reply_id,
			_ => self.id,
		}
	}
}
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthorPerms {
	Ignore,
	Answer,
	Obey,
}
impl AuthorPerms {
	pub fn should_reply(&self) -> bool {
		*self >= Self::Answer
	}
	pub fn should_obey(&self) -> bool {
		*self >= Self::Obey
	}
}

pub fn author_perms(msg: &EventWithContext<&MessageCreate>) -> AuthorPerms {
	if msg.author.bot {
		return AuthorPerms::Ignore;
	}
	let author_id = msg.author.id;
	if author_id == Id::new(310702108997320705) {
		return AuthorPerms::Obey;
	}
	if let Some(member) = msg.cache.member(FIRMAMENT_SERVER, author_id) {
		if member.roles().contains(&OBEY_ROLE) {
			return AuthorPerms::Obey;
		}
	}
	AuthorPerms::Answer
}

type EventFnInner = fn(EventContext) -> Box<dyn Future<Output = eyre::Result<()>> + Send>;

pub struct BoxedEventHandler(EventFnInner);

impl BoxedEventHandler {
	pub const fn new(handler: EventFnInner) -> BoxedEventHandler {
		BoxedEventHandler(handler)
	}

	pub async fn handle(&self, event_context: EventContext) -> eyre::Result<()> {
		let fut = self.0(event_context);
		let fut = Pin::from(fut);
		fut.await
	}
}

pub static FIRMAMENT_SERVER: Id<GuildMarker> = Id::new(1088154030628417616);
pub static OBEY_ROLE: Id<RoleMarker> = Id::new(1392489377699201086);

inventory::collect!(BoxedEventHandler);

#[macro_export]
macro_rules! handle_all {
	($handler:ident) => {
		::inventory::submit! {
			$crate::utils::BoxedEventHandler::new(|_event_context| ::std::boxed::Box::new($handler(_event_context)))
		}
	};
}

#[macro_export]
macro_rules! handle {
	($case:ident, $handler:expr) => {
		::inventory::submit! {
			$crate::utils::BoxedEventHandler::new(|_event_context| ::std::boxed::Box::new(async move {
				match _event_context.event.as_ref() {
					::twilight_gateway::Event::$case (_prop) => $handler($crate::EventWithContext {event: _prop, .._event_context}).await,
					_otherwise => Ok(())
				}
			}))
		}
	};
}

#[macro_export]
macro_rules! handle_message {
	($condition:ident, $handler:expr) => {
		$crate::handle!(MessageCreate, async |ctx: $crate::EventWithContext<
			&::twilight_model::gateway::payload::incoming::MessageCreate,
		>| {
			if !$crate::utils::author_perms(&ctx).$condition() {
				return Ok(());
			}
			$handler(ctx).await
		});
	};
}
