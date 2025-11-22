// @generated automatically by Diesel CLI.

diesel::table! {
    codebases (codebase_uuid) {
        codebase_uuid -> Uuid,
        branch -> Text,
        project_uuid -> Uuid,
        created_at -> Timestamptz,
        configuration_uuid -> Uuid,
    }
}

diesel::table! {
    commits (commit_hash) {
        #[max_length = 64]
        commit_hash -> Bpchar,
        commit_message -> Text,
        codebase_uuid -> Uuid,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    configurations (configuration_uuid) {
        configuration_uuid -> Uuid,
        project_uuid -> Uuid,
        configuration_data -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    file_records (id) {
        id -> Int8,
        file_path -> Text,
        codebase_uuid -> Uuid,
        uploaded_at -> Timestamptz,
        file_size -> Int8,
    }
}

diesel::table! {
    projects (project_uuid) {
        project_uuid -> Uuid,
        name -> Text,
        owner -> Text,
        created_at -> Timestamptz,
    }
}

diesel::joinable!(codebases -> configurations (configuration_uuid));
diesel::joinable!(codebases -> projects (project_uuid));
diesel::joinable!(commits -> codebases (codebase_uuid));
diesel::joinable!(configurations -> projects (project_uuid));
diesel::joinable!(file_records -> codebases (codebase_uuid));

diesel::allow_tables_to_appear_in_same_query!(
    codebases,
    commits,
    configurations,
    file_records,
    projects,
);
