use twilight_model::user::User;

pub trait UserExt {
    fn mention(&self) -> String;
}
impl UserExt for User {
    fn mention(&self) -> String {
        format!("<@{}>", self.id)
    }
}
