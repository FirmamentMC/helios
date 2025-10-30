use twilight_model::id::{Id, marker::UserMarker};

use crate::fixed_regex;

pub fn chomp_user(line: &str) -> Option<(Id<UserMarker>, &str)> {
	fixed_regex!(USER_REGEX = "<@([0-9]+)>");
	USER_REGEX.captures_at(line, 0).map(|it| {
		(
			Id::new((it.get(1).unwrap()).as_str().parse().unwrap()),
			line[it.get(0).unwrap().end()..].trim_start(),
		)
	})
}
