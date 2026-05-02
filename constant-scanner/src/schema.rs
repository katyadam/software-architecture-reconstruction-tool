// @generated automatically by Diesel CLI.

diesel::table! {
    commits (commit_hash) {
        #[max_length = 64]
        commit_hash -> Bpchar,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    constants (constant_uuid) {
        constant_uuid -> Uuid,
        #[max_length = 64]
        commit_hash -> Bpchar,
        name -> Text,
        value -> Text,
        created_at -> Timestamptz,
        source -> Nullable<Text>,
    }
}

diesel::joinable!(constants -> commits (commit_hash));

diesel::allow_tables_to_appear_in_same_query!(commits, constants,);
