use std::pin::Pin;

use twilight_model::user::User;

use crate::EventContext;

pub trait UserExt {
	fn mention(&self) -> String;
}
impl UserExt for User {
	fn mention(&self) -> String {
		format!("<@{}>", self.id)
	}
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
	($case:ident, $handler:ident) => {
		::inventory::submit! {
			$crate::utils::BoxedEventHandler::new(|_event_context| ::std::boxed::Box::new(async move {
				let $crate::EventWithContext {event: _event, client: _client } = _event_context;
				match _event.as_ref() {
					::twilight_gateway::Event::$case (_prop) => $handler($crate::EventWithContext::new(_prop, _client)).await,
					_otherwise => Ok(())
				}
			}))
		}
	};
}
