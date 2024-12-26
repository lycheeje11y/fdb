// @generated automatically by Diesel CLI.

diesel::table! {
    friends (id) {
        id -> Integer,
        name -> Text,
        email -> Text,
    }
}
