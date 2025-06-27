use std::{path::PathBuf, sync::Arc};

use cow_hashmap::CowHashMap;
use eyre::OptionExt;
use tokio::sync::Mutex;
use twilight_model::gateway::payload::incoming::MessageCreate;

use crate::{EventWithContext, handle_message};

handle_message!(should_reply, on_message_send_tags);
handle_message!(should_obey, on_message_edit_tags);

async fn on_message_send_tags(context: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	let message = context.event;
	let mut rest = message.content.as_str();
	let handler = tag_handler().await;
	while !rest.is_empty() {
		let Some(next) = rest.find("!") else {
			break;
		};
		rest = &rest[next + 1..];
		let command = read_till(rest, " ");
		let Some(tag) = handler.tags.get(command) else {
			continue;
		};
		context.reply().content(&tag).await?;
	}
	Ok(())
}

async fn on_message_edit_tags(context: EventWithContext<&MessageCreate>) -> eyre::Result<()> {
	let content = &context.content;
	let Some(subcommand) = content.strip_prefix("!tag ") else {
		return Ok(());
	};
	let handler = tag_handler().await;
	if subcommand == "list" {
		let content = handler
			.tags
			.keys()
			.map(|x| format!("`{x}`"))
			.intersperse(", ".to_owned())
			.collect::<String>();
		context.reply().content(&content).await?;
		return Ok(());
	}
	if let Some(rest) = subcommand.strip_prefix("add ") {
		let Some((key, reply)) = rest.split_once(' ') else {
			context
				.reply()
				.content("use: !tag add <name> <content>")
				.await?;
			return Ok(());
		};
		handler.write_tag(key, Some(reply)).await?;
		let text = format!("created tag `{}`", key);
		context.reply().content(&text).await?;
		return Ok(());
	}
	if let Some(rest) = subcommand.strip_prefix("del ") {
		handler.write_tag(rest, None).await?;
		let text = format!("deleted tag `{}`", rest);
		context.reply().content(&text).await?;
		return Ok(());
	}
	context
		.reply()
		.content("unknown subcommand. valid options are add, list, del")
		.await?;
	Ok(())
}

async fn tag_handler() -> Arc<TagHandler> {
	static _TAG_HANDLER: Mutex<Option<Arc<TagHandler>>> = Mutex::const_new(None);
	let mut opt = _TAG_HANDLER.lock().await;
	if let Some(handler) = opt.as_ref() {
		return handler.clone();
	}
	let handler = TagHandler::load().await.unwrap();
	let handler = Arc::new(handler);
	opt.replace(handler.clone());
	handler
}

struct TagHandler {
	tags: CowHashMap<Arc<str>, String>,
	write_handle: Mutex<PathBuf>,
}
impl TagHandler {
	async fn write_tag(&self, key: &str, reply: Option<&str>) -> eyre::Result<()> {
		let path = self.write_handle.lock().await;
		let file_path = path.join(format!("{key}.md"));
		match reply {
			Some(content) => {
				tokio::fs::write(file_path, content).await?;
				self.tags.insert(key.into(), content.into());
			}
			None => {
				tokio::fs::remove_file(file_path).await?;
				self.tags.remove(key);
			}
		}
		Ok(())
	}

	async fn load() -> eyre::Result<TagHandler> {
		let path = PathBuf::from("tags");
		let mut dir = tokio::fs::read_dir(&path).await?;
		let tags = cow_hashmap::CowHashMap::new();
		while let Some(file) = dir.next_entry().await? {
			let name = file.file_name();
			let name = name
				.to_str()
				.ok_or_eyre("could not parse os string in tags")?;
			let content = tokio::fs::read_to_string(file.path()).await?;
			let (name, content) = parse_tag(name, content);
			tags.insert(name.into(), content.into());
		}

		Ok(TagHandler {
			tags: tags,
			write_handle: Mutex::new(path),
		})
	}
}

// <P: Pattern>
fn read_till<'a>(name: &'a str, filter: &'static str) -> &'a str {
	match name.find(filter) {
		Some(index) => &name[..index],
		None => name,
	}
}

fn parse_tag(name: &str, content: String) -> (&str, String) {
	(read_till(name, "."), content)
}
