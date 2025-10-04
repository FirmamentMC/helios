use std::sync::LazyLock;

use regex::Regex;
use twilight_model::id::{Id, marker::UserMarker};

pub fn chomp_user(line: &str) -> Option<(Id<UserMarker>, &str)> {
	static USER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("<@([0-9]+)>").unwrap());
	USER_REGEX.captures_at(line, 0).map(|it| {
		(
			Id::new((it.get(1).unwrap()).as_str().parse().unwrap()),
			line[it.get(0).unwrap().end()..].trim_start(),
		)
	})
}
