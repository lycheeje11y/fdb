use diesel::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Selectable, Queryable)]
#[diesel(table_name = crate::schema::friends)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Friend {
    id: i32,
    name: String,
    email: String,
}

#[derive(Insertable, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::friends)]
pub struct NewFriend {
    name: String,
    email: String,
}
